use std::fmt;

use crate::crc::{Crc16Reader, Crc8Reader};
use crate::io::{BitStream, ReadBuffer};
use crate::{audio, errors, Result};

use super::decoder;

enum BlockStrategy {
    Fixed,
    Variable,
}

enum BlockType {
    FrameNumber(u32),
    SampleNumber(u64),
}

#[derive(Clone, Copy, Debug)]
enum ChannelType {
    /// The `n: u8` channels are coded as-is.
    Independent(u8),
    /// Channel 0 is the left channel, channel 1 is the side channel.
    LeftSideStereo,
    /// Channel 0 is the side channel, channel 1 is the right channel.
    RightSideStereo,
    /// Channel 0 is the mid channel, channel 1 is the side channel.
    MidSideStereo,
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

struct FrameHeader {
    pub block_type: BlockType,
    pub block_size: u16,
    pub sample_rate: u32,
    pub channel_type: ChannelType,
    pub bits_per_sample: u32,
}

impl FrameHeader {
    pub fn number_channels(&self) -> u8 {
        match self.channel_type {
            ChannelType::Independent(n) => n,
            _ => 2,
        }
    }
}

#[derive(Debug)]
enum SubFrameType {
    Constant,
    Verbatim,
    FixedLinear(u8),
    Lpc(u8),
}

/// represent a block of decoded samples from a frame
#[allow(dead_code)]
pub struct Block {
    /// index of the first sample of this block w.r.t total samples
    first_sample_index: u64,
    /// number of channel independent samples in this block
    block_size: u32,
    /// number of channels in this block
    no_channels: u32,
    /// bits pr sample
    bits_per_sample: u32,
    /// decoded samples with channels one after another
    buffer: Vec<i32>,
}

impl Block {
    fn new(sample_index: u64, block_size: u32, bps: u32, buffer: Vec<i32>) -> Block {
        Block {
            first_sample_index: sample_index,
            block_size,
            no_channels: buffer.len() as u32 / block_size,
            bits_per_sample: bps,
            buffer,
        }
    }

    pub fn empty() -> Block {
        Block {
            first_sample_index: 0,
            block_size: 0,
            no_channels: 0,
            bits_per_sample: 0,
            buffer: Vec::with_capacity(0),
        }
    }

    #[inline(always)]
    pub fn total_samples(&self) -> u32 {
        self.block_size
    }

    #[inline(always)]
    pub fn num_channels(&self) -> u32 {
        self.no_channels
    }

    #[inline(always)]
    pub fn bits_per_sample(&self) -> u32 {
        self.bits_per_sample
    }

    /// returns the underlying buffer which stores sample
    #[inline(always)]
    pub fn into_buffer(self) -> Vec<i32> {
        self.buffer
    }

    /// return the decoded sample from the buffer
    #[inline(always)]
    pub fn get_sample(&self, current_channel: u32, samples_read: u32) -> i32 {
        self.buffer[current_channel as usize * self.block_size as usize + samples_read as usize]
    }
}

/// Converts a buffer with left samples and a side channel in-place to left ++ right.
fn decode_left_side(buffer: &mut [i32]) {
    let block_size = buffer.len() / 2;
    let (mids, sides) = buffer.split_at_mut(block_size);
    for (fst, snd) in mids.iter_mut().zip(sides) {
        let left = *fst;
        let side = *snd;

        // Left is correct already, only the right channel needs to be decoded.
        // side = left - right => right = left - side.
        *snd = left.wrapping_sub(side);
    }
}

/// Converts a buffer with right samples and a side channel in-place to left ++ right.
fn decode_right_side(buffer: &mut [i32]) {
    let block_size = buffer.len() / 2;
    let (mids, sides) = buffer.split_at_mut(block_size);
    for (fst, snd) in mids.iter_mut().zip(sides) {
        let right = *snd;
        let side = *fst;

        // Right is correct already, only the left channel needs to be decoded.
        // side = left - right => left = right + side.
        *fst = right.wrapping_add(side);
    }
}

/// Converts a buffer with mid samples and a side channel in-place to left ++ right.
fn decode_mid_side(buffer: &mut [i32]) {
    let block_size = buffer.len() / 2;
    let (mids, sides) = buffer.split_at_mut(block_size);
    for (fst, snd) in mids.iter_mut().zip(sides) {
        let mid = *fst;
        let side = *snd;

        // mid = (left + right) / 2
        // side = left - right
        // Double mid first, and then correct for truncated rounding that
        // will have occurred if side is odd.
        let mid = mid.wrapping_mul(2) | (side & 1);

        *fst = mid.wrapping_add(side) / 2;
        *snd = mid.wrapping_sub(side) / 2;
    }
}

// read variable length encoded int
// It is encoded utf-8 style but can go up to 36bits
fn read_utf8_coded_int<R: ReadBuffer>(crc_reader: &mut Crc8Reader<R>) -> Result<u64> {
    // The number of consecutive 1s followed by a 0 is the number of extra bytes to read. i.e
    // 0xxxxxxx -> 0 extra byte to read
    // 10xxxxxx -> Invalid for first byte, it is a followup byte
    // 110xxxxx -> 1 extra byte
    // 1110xxxx -> 2 extra byte
    // ...
    // see this https://en.wikipedia.org/wiki/UTF-8 for detailed explanation
    let first = crc_reader.read_u8()?;

    let mut read_extra = 0u8;
    let mut mask_mark = 0b1000_0000u8;
    let mut mask_data = 0b0111_1111u8;

    while first & mask_mark != 0 {
        read_extra += 1;
        mask_mark >>= 1;
        mask_data >>= 1;
    }

    // 10xxxxxx -> is invalid
    if read_extra > 0 {
        if read_extra == 1 {
            return errors::parse_error("Invalid utf8 encoding for integer");
        } else {
            read_extra -= 1;
        }
    }

    // Each additional byte will yield 6 extra bits, so shift the most
    // significant bits into the correct position.
    let mut result = ((first & mask_data) as u64) << (6 * read_extra);
    for i in (0..read_extra as i16).rev() {
        let byte = crc_reader.read_u8()?;

        // The two most significant bits _must_ be 10.
        if byte & 0b1100_0000 != 0b1000_0000 {
            return errors::parse_error("invalid utf8 encoding for integer");
        }
        result |= ((byte & 0b0011_1111) as u64) << (6 * i as usize);
    }
    Ok(result)
}

// See https://xiph.org/flac/format.html#frame_header for header info
fn read_frame_header<R: ReadBuffer>(
    crc_reader: &mut Crc8Reader<R>,
    audio_info: &audio::AudioInfo,
    sync_code: u16,
) -> Result<FrameHeader> {
    // check sync code
    // The first 14 bits must be 11111111111110.
    if sync_code & 0b1111_1111_1111_1100 != 0b1111_1111_1111_1000 {
        return errors::parse_error("frame sync code incorrect");
    }

    // According to format spec, next value must be 0, 1 is reserved for future use
    // when format will get changed, hence throwing unsupported when encountering it
    if sync_code & 0b0000_0000_0000_0010 != 0 {
        return errors::unsupported_error("invalid frame header, encountered reserved value");
    }

    // The final bit determines the blocking strategy.
    let blocking_strategy = if sync_code & 0b0000_0000_0000_0001 == 0 {
        BlockStrategy::Fixed
    } else {
        BlockStrategy::Variable
    };

    // next 4 bits determine block size and next 4 determine sample rate
    let bs_sr = crc_reader.read_u8()?;

    let mut block_size = 0u16;
    let mut read_bs_last = 0u8;

    match bs_sr >> 4 {
        0b0000 => {
            return errors::unsupported_error("invalid frame header, encountered reserved value")
        }
        0b0001 => block_size = 192,
        n if (0b0010..=0b0101).contains(&n) => block_size = 576 * (1 << (n - 2) as usize),
        0b0110 => read_bs_last = 1, // read 8 bit at end of header
        0b0111 => read_bs_last = 2, // read 16 bit at end of header
        n => block_size = 256 * (1 << (n - 8) as usize),
    }

    let mut sample_rate = 0;
    let mut read_sr_last = 0u8;

    // matching sample rate
    match bs_sr & 0b00001111 {
        0b0000 => sample_rate = audio_info.sample_rate, // get from streaminfo block'.
        0b0001 => sample_rate = 88200,
        0b0010 => sample_rate = 176400,
        0b0011 => sample_rate = 192000,
        0b0100 => sample_rate = 8000,
        0b0101 => sample_rate = 16000,
        0b0110 => sample_rate = 22050,
        0b0111 => sample_rate = 24000,
        0b1000 => sample_rate = 32000,
        0b1001 => sample_rate = 44100,
        0b1010 => sample_rate = 48000,
        0b1011 => sample_rate = 96000,
        0b1100 => read_sr_last = 1, // Read 8bit sample rate from end of header.
        0b1101 => read_sr_last = 2, // Read 16bit sample rate from end of header.
        0b1110 => read_sr_last = 3, // Read 16bit sample rate in tens from end of header.
        _ => return errors::parse_error("invalid frame header"),
    }

    // Next 4 bits is for no of channels, then bits per sample and then reserved bit
    let ch_bps_r = crc_reader.read_u8()?;

    let channel_type = match ch_bps_r >> 4 {
        n if n < 8 => ChannelType::Independent(n + 1),
        0b1000 => ChannelType::LeftSideStereo,
        0b1001 => ChannelType::RightSideStereo,
        0b1010 => ChannelType::MidSideStereo,
        _ => return errors::unsupported_error("invalid frame header, encountered reserved value"),
    };
    // The next three bits indicate bits per sample.
    let bps = match (ch_bps_r & 0b0000_1110) >> 1 {
        0b000 => audio_info.bits_per_sample,
        0b001 => 8,
        0b010 => 12,
        0b100 => 16,
        0b101 => 20,
        0b110 => 24,
        _ => return errors::unsupported_error("invalid frame header, encountered reserved value"),
    };

    // The last bit is reserved and should have value 0 .
    if ch_bps_r & 0b0000_0001 != 0 {
        return errors::unsupported_error("invalid frame header, encountered reserved value");
    }

    let block_type = match blocking_strategy {
        BlockStrategy::Fixed => BlockType::FrameNumber(read_utf8_coded_int(crc_reader)? as u32),
        BlockStrategy::Variable => BlockType::SampleNumber(read_utf8_coded_int(crc_reader)?),
    };

    // read 8bit block size - 1 at last
    if read_bs_last == 1 {
        block_size = crc_reader.read_u8()? as u16 + 1;
    }
    // read 16bit block size - 1 at last
    if read_bs_last == 2 {
        block_size = crc_reader.read_be_u16()? + 1;
    }

    // next read sample rate 8bit or 16bit
    if read_sr_last == 1 {
        sample_rate = crc_reader.read_u8()? as u32;
    }
    if read_sr_last == 2 {
        sample_rate = crc_reader.read_be_u16()? as u32;
    }
    if read_sr_last == 3 {
        sample_rate = crc_reader.read_be_u16()? as u32 * 10;
    }

    // Now just check crc
    // read the 8bit crc and match it with computed crc
    let crc_computed = crc_reader.crc();
    if crc_computed != crc_reader.get_input().read_u8()? {
        return errors::parse_error("CRC match failed, Invalid frame");
    }

    Ok(FrameHeader {
        block_type,
        block_size,
        sample_rate,
        channel_type,
        bits_per_sample: bps,
    })
}

// fix current buffer capacity to accommodate total samples for this block
fn correct_buffer_len(mut buffer: Vec<i32>, new_len: usize) -> Vec<i32> {
    if buffer.len() != new_len {
        if buffer.capacity() < new_len {
            buffer = vec![0; new_len];
        } else {
            buffer.resize(new_len, 0);
        }
    }
    buffer
}

fn decode_subframe<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    bps: u32,
    buffer: &mut [i32],
) -> Result<()> {
    // read the padding bit
    if bitstream.read_bit()? {
        return errors::parse_error("subframe sync code incorrect");
    }

    // read subframe type
    let subframe_type = match bitstream.read_len_u8(6)? {
        0 => SubFrameType::Constant,
        1 => SubFrameType::Verbatim,
        n if (n & 0b11_1110 == 0b00_0010)
            || (n & 0b11_1100 == 0b00_0100)
            || (n & 0b11_0000 == 0b01_0000) =>
        {
            return errors::unsupported_error(
                "invalid subframe header, encountered reserved value",
            );
        }
        n if (n & 0b11_1000 == 0b00_1000) => {
            let order = n & 0b00_0111;

            // A fixed frame has order up to 4, other bit patterns are reserved.
            if order > 4 {
                return errors::unsupported_error("fixed linear should not have order more than 4");
            }

            SubFrameType::FixedLinear(order)
        }
        n => SubFrameType::Lpc((n & 0b01_1111) + 1),
    };

    let wasted_bps = if bitstream.read_bit()? {
        1 + bitstream.read_unary()?
    } else {
        0
    };
    if wasted_bps > bps {
        return errors::parse_error("subframe has no non-wasted bits");
    }

    let sf_bps = bps - wasted_bps;

    match subframe_type {
        SubFrameType::Constant => decoder::decode_constant::<R>(bitstream, sf_bps, buffer)?,
        SubFrameType::Verbatim => decoder::decode_verbatim::<R>(bitstream, sf_bps, buffer)?,
        SubFrameType::FixedLinear(order) => {
            decoder::decode_fixed_linear::<R>(bitstream, sf_bps, order as usize, buffer)?
        }
        SubFrameType::Lpc(order) => {
            decoder::decode_lpc::<R>(bitstream, sf_bps, order as usize, buffer)?
        }
    }

    if wasted_bps > 0 {
        for s in buffer {
            // make a no panic left shift i.e *s = s << wasted_bps
            *s = s.wrapping_shl(wasted_bps);
        }
    }

    Ok(())
}

pub fn decode_next_frame<R: ReadBuffer>(
    input: &mut R,
    mut block_buffer: Vec<i32>,
    audio_info: &audio::AudioInfo,
) -> Option<Result<Block>> {
    // create crc16 reader
    let mut crc16reader = Crc16Reader::new(input);

    // decode frame header
    // will keep track of crc state in frame header
    let mut crc8reader = Crc8Reader::new(&mut crc16reader);
    let sync_code = match crc8reader.read_be_u16() {
        Ok(sync_code) => sync_code,
        Err(_) => return None,
    };
    let frame_header = otry!(read_frame_header(&mut crc8reader, audio_info, sync_code));

    let bs = frame_header.block_size as usize;
    let total_samples = frame_header.number_channels() as usize * bs;
    block_buffer = correct_buffer_len(block_buffer, total_samples);

    // now buffer reading is not byte aligned anymore, hence BitStream is used
    let mut bitstream = BitStream::new(&mut crc16reader);

    // decode subframes and fill buffer
    match frame_header.channel_type {
        ChannelType::Independent(n_ch) => {
            for ch in 0..n_ch as usize {
                otry!(decode_subframe(
                    &mut bitstream,
                    frame_header.bits_per_sample,
                    &mut block_buffer[ch * bs..(ch + 1) * bs]
                ));
            }
        }
        ChannelType::LeftSideStereo => {
            // The side channel has one extra bit per sample.
            otry!(decode_subframe(
                &mut bitstream,
                frame_header.bits_per_sample,
                &mut block_buffer[..bs]
            ));
            otry!(decode_subframe(
                &mut bitstream,
                frame_header.bits_per_sample + 1,
                &mut block_buffer[bs..bs * 2]
            ));

            // Then decode the side channel into the right channel.
            decode_left_side(&mut block_buffer[..bs * 2]);
        }
        ChannelType::RightSideStereo => {
            // The side channel has one extra bit per sample.
            otry!(decode_subframe(
                &mut bitstream,
                frame_header.bits_per_sample + 1,
                &mut block_buffer[..bs]
            ));
            otry!(decode_subframe(
                &mut bitstream,
                frame_header.bits_per_sample,
                &mut block_buffer[bs..bs * 2]
            ));

            // Then decode the side channel into the left channel.
            decode_right_side(&mut block_buffer[..bs * 2]);
        }
        ChannelType::MidSideStereo => {
            // Decode mid as the first channel, then side with one
            // extra bits per sample.
            otry!(decode_subframe(
                &mut bitstream,
                frame_header.bits_per_sample,
                &mut block_buffer[..bs]
            ));
            otry!(decode_subframe(
                &mut bitstream,
                frame_header.bits_per_sample + 1,
                &mut block_buffer[bs..bs * 2]
            ));

            // Then decode mid-side channel into left-right.
            decode_mid_side(&mut block_buffer[..bs * 2]);
        }
    }

    // check crc-16
    // match calculated crc == encoded crc
    if crc16reader.crc() != otry!(crc16reader.read_be_u16()) {
        return Some(errors::parse_error("frame CRC mismatch"));
    }

    let frame_fsi = match frame_header.block_type {
        BlockType::FrameNumber(fno) => frame_header.block_size as u64 * fno as u64,
        BlockType::SampleNumber(sno) => sno,
    };

    Some(Ok(Block::new(
        frame_fsi,
        frame_header.block_size as u32,
        frame_header.bits_per_sample,
        block_buffer,
    )))
}
