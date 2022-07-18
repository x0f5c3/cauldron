use crate::io::{BitStream, ReadBuffer};
use crate::{errors, Result};

use super::types::*;

/// Bit-rate lookup table for MPEG version 1 layer 3.
static BIT_RATES_MPEG1_L3: [u32; 15] = [
    0, 32_000, 40_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000, 128_000, 160_000, 192_000,
    224_000, 256_000, 320_000,
];

/// Bit-rate lookup table for MPEG version 2 & 2.5 audio layer 3.
static BIT_RATES_MPEG2_L3: [u32; 15] = [
    0, 8_000, 16_000, 24_000, 32_000, 40_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000,
    128_000, 144_000, 160_000,
];

/// represent a block of decoded samples from a frame
pub struct Block {
    /// number of channel independent samples in this block
    block_size: u32,
    /// number of channels in this block
    no_channels: u32,
    /// bits pr sample
    bits_per_sample: u32,
    /// decoded samples with channels one after another
    buffer: Vec<f32>,
}

impl Block {
    fn new(block_size: u32, bps: u32, buffer: Vec<f32>) -> Block {
        Block {
            block_size,
            no_channels: buffer.len() as u32 / block_size,
            bits_per_sample: bps,
            buffer,
        }
    }

    pub fn empty() -> Block {
        Block {
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
    pub fn into_buffer(self) -> Vec<f32> {
        self.buffer
    }

    /// return the decoded sample from the buffer
    #[inline(always)]
    pub fn get_sample(&self, current_channel: u32, samples_read: u32) -> f32 {
        self.buffer[current_channel as usize * self.block_size as usize + samples_read as usize]
    }
}

/// Used for Internal decoding
///
/// Keep bit reservoir
pub struct DecoderState {
    frame_buffer: [u8; 2048],
    frame_buffer_len: usize,
}

impl DecoderState {
    pub fn new() -> Self {
        DecoderState {
            frame_buffer: [0; 2048],
            frame_buffer_len: 0,
        }
    }

    fn fill_reservoir_buffer<R: ReadBuffer>(
        &mut self,
        input: &mut R,
        main_data_begin: usize,
        main_data_size: usize,
    ) -> Result<&[u8]> {
        let main_data_actual_size = main_data_begin + main_data_size;
        if main_data_actual_size > 2048 {
            return errors::parse_error("main_data length greater than reservoir buffer");
        }

        // shift the actual used data to start of the buffer
        if main_data_begin <= self.frame_buffer_len {
            self.frame_buffer.copy_within(
                self.frame_buffer_len - main_data_begin..self.frame_buffer_len,
                0,
            );
        } else {
            // this could be because we haven't buffered enough data or
            // `main_data_begin` was really invalid.
            // For now just throw an error.
            return errors::parse_error("invalid main data begin offset");
        }

        // add the main_data bytes of this frame to reservoir buffer
        input.read_into(&mut self.frame_buffer[main_data_begin..main_data_actual_size])?;
        self.frame_buffer_len = main_data_actual_size;

        Ok(&self.frame_buffer[0..main_data_actual_size])
    }
}

fn sync_frame<R: ReadBuffer>(input: &mut R) -> Result<u32> {
    let mut sync = 0u32;

    // Synchronize stream to the next frame using the sync word.
    // The MP3 frame header always starts with 0xffe (11 consecutive 1 bits)
    while (sync & 0xffe0_0000) != 0xffe0_0000 {
        sync = sync.wrapping_shl(8) | input.read_u8()? as u32;
    }

    Ok(sync)
}

/// Mp3 header is as follows [4 bytes]:
///
/// AAAAAAAA AAABBCCD EEEEFFGH IIJJKLMM
///
/// A => sync bits [should be all 1]  | H => private bit
/// B => mpeg version                 | I => channel mode
/// C => layer                        | J => mode extension
/// D => is crc present               | K => copyright
/// E => bit rate                     | L => original
/// F => sampling rate                | M => emphasis while encoding
/// G => padding bit                  |
///
fn read_header<R: ReadBuffer>(input: &mut R, header: u32) -> Result<FrameHeader> {
    let mut frame_header = FrameHeader {
        version: MPEGVersion::MPEG1,
        bitrate: 0,
        sample_rate: 0,
        channel_mode: ChannelMode::Mono,
        emphasis: Emphasis::None,
        has_padding: false,
        frame_size: 0,
        crc: None,
    };

    frame_header.version = match (header & 0x0018_0000) >> 19 {
        0b00 => MPEGVersion::MPEG2p5,
        0b10 => MPEGVersion::MPEG2,
        0b11 => MPEGVersion::MPEG1,
        _ => return errors::parse_error("invalid MPEG version"),
    };

    if (header & 0x6_0000) >> 17 != 1 {
        return errors::unsupported_error("only layer 3 is supported");
    }

    frame_header.bitrate = match (header & 0x0_f000) >> 12 {
        0b0000 => return errors::unsupported_error("free bitrate is not supported"),
        0b1111 => return errors::parse_error("unsupported bitrate"),
        n => {
            if frame_header.version == MPEGVersion::MPEG1 {
                BIT_RATES_MPEG1_L3[n as usize]
            } else {
                BIT_RATES_MPEG2_L3[n as usize]
            }
        }
    };

    frame_header.sample_rate = match ((header & 0x0_0c00) >> 10, frame_header.version) {
        (0b00, MPEGVersion::MPEG1) => 44_100,
        (0b01, MPEGVersion::MPEG1) => 48_000,
        (0b10, MPEGVersion::MPEG1) => 32_000,
        (0b00, MPEGVersion::MPEG2) => 22_050,
        (0b01, MPEGVersion::MPEG2) => 24_000,
        (0b10, MPEGVersion::MPEG2) => 16_000,
        (0b00, MPEGVersion::MPEG2p5) => 11_025,
        (0b01, MPEGVersion::MPEG2p5) => 12_000,
        (0b10, MPEGVersion::MPEG2p5) => 8_000,
        _ => return errors::parse_error("Invalid sample rate."),
    };

    frame_header.channel_mode = match (header & 0x0_00c0) >> 6 {
        0b00 => ChannelMode::Stereo,
        0b10 => ChannelMode::DualMono,
        0b11 => ChannelMode::Mono,
        0b01 => ChannelMode::JointStereo {
            mid_side: false,
            intensity: false,
        },
        _ => return errors::parse_error("unsupported channel mode"),
    };

    frame_header.emphasis = match header & 0x0_0003 {
        0b00 => Emphasis::None,
        0b01 => Emphasis::Fifty15,
        0b11 => Emphasis::CcitJ17,
        _ => return errors::parse_error("invalid emphasis, found reserved bits"),
    };

    frame_header.has_padding = (header & 0x0_0200) >> 9 == 1;

    // if crc is present read it as 16bit big endian
    // https://www.codeproject.com/Articles/8295/MPEG-Audio-Frame-Header#CRC
    if (header & 0x1_0000) == 0 {
        frame_header.crc = Some(input.read_be_u16()?);
    }

    // calculate frame size
    let bits_per_sample = match frame_header.version {
        MPEGVersion::MPEG1 => 144,
        _ => 72,
    };
    frame_header.frame_size = (bits_per_sample * frame_header.bitrate / frame_header.sample_rate
        + if frame_header.has_padding { 1 } else { 0 }
        - if frame_header.crc.is_some() { 2 } else { 0 }
        - 4) as usize; // header bytes

    Ok(frame_header)
}

fn read_granule_channel_side_info<R: ReadBuffer>(
    bs: &mut BitStream<R>,
    is_mpeg1: bool,
    granule_channel_info: &mut GranuleChannel,
) -> Result<()> {
    granule_channel_info.part2_3_length = bs.read_len_u16(12)?;
    granule_channel_info.big_values = bs.read_len_u16(9)?;

    // check max value of big_values <= 288
    if granule_channel_info.big_values > 288 {
        return errors::parse_error("Granules big values > 288");
    }

    granule_channel_info.global_gain = bs.read_len_u8(8)?;
    granule_channel_info.scalefac_compress_len = if is_mpeg1 {
        bs.read_len_u16(4)
    } else {
        bs.read_len_u16(9)
    }?;

    let window_switching_flag = bs.read_bit()?;
    if window_switching_flag {
        let block_type_enc = bs.read_len_u8(2)?;
        let is_mixed = bs.read_bit()?;

        granule_channel_info.block_type = match block_type_enc {
            // Long block types are not allowed with window switching.
            0b00 => return errors::parse_error("Invalid block_type"),
            0b01 => BlockType::Start,
            0b10 => BlockType::Short { is_mixed },
            0b11 => BlockType::End,
            _ => unreachable!(),
        };
        for i in 0..2 {
            granule_channel_info.table_select[i] = bs.read_len_u8(5)?;
        }
        for i in 0..3 {
            granule_channel_info.subblock_gain[i] = bs.read_len_u8(3)? as u8;
        }
        // region count set in terms of long block cb's/bands
        // r1 set so r0+r1+1 = 21 (lookup produces 576 bands)
        // bt=1 or 3       54 samples
        // bt=2 mixed=0    36 samples
        // bt=2 mixed=1    54 (8 long sf) samples? or maybe 36
        if is_mpeg1 {
            granule_channel_info.region0_count = 7;
        } else {
            granule_channel_info.region0_count = match granule_channel_info.block_type {
                BlockType::Short { is_mixed: false } => 5,
                _ => 7,
            };
        }
        granule_channel_info.region1_count = 20 - granule_channel_info.region0_count;
    } else {
        // If window switching is not used, the block type is always Long.
        granule_channel_info.block_type = BlockType::Long;

        for i in 0..3 {
            granule_channel_info.table_select[i] = bs.read_len_u8(5)?;
        }

        granule_channel_info.region1_count = bs.read_len_u8(4)?;
        granule_channel_info.region1_count = bs.read_len_u8(3)?;
    }

    granule_channel_info.preflag = if is_mpeg1 {
        bs.read_bit()?
    } else {
        // Pre-flag is determined implicitly for MPEG2: ISO/IEC 13818-3 section 2.4.3.4.
        granule_channel_info.scalefac_compress_len >= 500
    };

    granule_channel_info.scalefac_scale = bs.read_bit()?;
    granule_channel_info.count1table_select = bs.read_bit()?;

    Ok(())
}

fn read_side_info<R: ReadBuffer>(input: &mut R, frame_header: &FrameHeader) -> Result<FrameInfo> {
    let mut frame_info: FrameInfo = Default::default();
    let mut input_stream = BitStream::new(input);

    let num_channels = frame_header.num_channels();
    let is_mpeg1 = frame_header.version == MPEGVersion::MPEG1;

    if is_mpeg1 {
        frame_info.main_data_begin = input_stream.read_len_u16(9)?;
        // skip private bits
        if num_channels == 1 {
            input_stream.skip_len_u8(5)?;
        } else {
            input_stream.skip_len_u8(3)?;
        }

        // read scfsi
        for scfsi in &mut frame_info.scfsi[..num_channels] {
            for band in scfsi.iter_mut() {
                *band = input_stream.read_bit()?;
            }
        }
    } else {
        frame_info.main_data_begin = input_stream.read_len_u16(8)?;
        if num_channels == 1 {
            input_stream.skip_len_u8(1)?;
        } else {
            input_stream.skip_len_u8(2)?;
        }
    }

    for granule in &mut frame_info.granules[..frame_header.num_granules()] {
        for channel_frame_info in &mut granule.channels[0..num_channels] {
            read_granule_channel_side_info(&mut input_stream, is_mpeg1, channel_frame_info)?;
        }
    }

    if !input_stream.is_aligned() {
        return errors::parse_error("unable to read side info properly");
    }

    Ok(frame_info)
}

/// It contains scale factors and huffman coded bits
///
/// Each granule and channel data is coded separately except in joint stereo mode
///
/// Main Data
/// |___Granule0
/// |   |__LeftChannel
/// |   |   |__ScaleFactor
/// |   |   |__Huffman Coded bits
/// |   |__RightChannel
/// |      |__ScaleFactor
/// |      |__Huffman Coded bits
/// |
/// |___Granule1
/// |   |....
fn read_main_data<R: ReadBuffer>(
    input: &mut R,
    decoder_state: &mut DecoderState,
    frame_header: &FrameHeader,
    frame_info: &mut FrameInfo,
) -> Result<()> {
    let main_data_size = frame_header.frame_size - frame_header.side_data_len();

    // fill the decoder state buffer with main_data bytes
    let buffer = decoder_state.fill_reservoir_buffer(
        input,
        frame_info.main_data_begin as usize,
        main_data_size,
    )?;

    for g in 0..frame_header.num_granules() {
        for c in 0..frame_header.num_channels() {
            // read scale factors
            if frame_header.version == MPEGVersion::MPEG1 {
                read_mpeg1_scale_factors(buffer, &mut frame_info.granules[g].channels[c])?;
            } else {
                read_mpeg2_scale_factors(
                    buffer,
                    c == 1 && frame_header.is_intensity_stereo(),
                    &mut frame_info.granules[g].channels[c],
                )?;
            }
            // read huffman coded bits
        }
    }

    Ok(())
}

fn read_mpeg1_scale_factors(_buffer: &[u8], _channel_info: &mut GranuleChannel) -> Result<()> {
    Ok(())
}

fn read_mpeg2_scale_factors(
    _buffer: &[u8],
    _intensity_stereo_channel: bool,
    _channel_info: &mut GranuleChannel,
) -> Result<()> {
    Ok(())
}

/// takes input stream and returns a block of pcm samples
///
/// -----------------    ----------------     --------------------
/// - Decoding bit  ----\- Inverse      -----\- Synthesis Filter -
/// - stream        ----/- Quantization -----/- Bank             -
/// -----------------    ----------------     --------------------
///
/// Frame contains following data:
/// ---------------------------------------------------------
/// | Header | CRC | Side Info | Main Data | Ancillary Data |
/// ---------------------------------------------------------
///
/// Each frame contains 1152 pcm encoded samples.
pub fn decode_next_frame<R: ReadBuffer>(
    input: &mut R,
    decoder_state: &mut DecoderState,
    mut block_buffer: Vec<f32>,
) -> Option<Result<Block>> {
    let header = match sync_frame(input) {
        Ok(h) => h,
        Err(_) => return None,
    };

    let frame_header = otry!(read_header(input, header));
    let mut frame_info = otry!(read_side_info(input, &frame_header));

    // allocate block buffer if empty
    if block_buffer.is_empty() {
        block_buffer = vec![0.0; 576 * frame_header.num_granules() * frame_header.num_channels()];
    }

    otry!(read_main_data(
        input,
        decoder_state,
        &frame_header,
        &mut frame_info
    ));

    Some(Ok(Block::new(
        576 * frame_header.num_granules() as u32,
        32,
        block_buffer,
    )))
}
