mod decoder;
mod frame;

use super::io::{AudioInputStream, AudioReader, AudioSamplesIterator, ReadBuffer, Sample};
use super::{audio, codecs, errors, Result};

const FLAC_MARKER: &[u8; 4] = b"fLaC";

pub struct FlacReader {
    reader: AudioInputStream,
    block_size: (u16, u16),
    frame_size: (u32, u32),
    md5: [u8; 16],
}

impl FlacReader {
    pub fn new(reader: AudioInputStream) -> Result<Box<Self>> {
        Ok(Box::new(FlacReader {
            reader,
            block_size: (0, 0),
            frame_size: (0, 0),
            md5: [0u8; 16],
        }))
    }

    // https://xiph.org/flac/format.html#metadata_block_streaminfo
    fn read_stream_info(&mut self, length: u32) -> Result<audio::AudioInfo> {
        if length != 34 {
            return errors::parse_error("stream_info block should have fixed length of 34 bytes");
        }

        // read block size
        // min block size should be 16 and must not be greater than max block size
        self.block_size = (self.reader.read_be_u16()?, self.reader.read_be_u16()?);
        if self.block_size.0 < 16 {
            return errors::parse_error("block size must be at least 16");
        }
        if self.block_size.0 > self.block_size.1 {
            return errors::parse_error("inconsistent block size, min block size > max block size");
        }

        // read frame size
        // if frame size values are known min frame size should not be greater than max frame size
        self.frame_size = (self.reader.read_be_u24()?, self.reader.read_be_u24()?);
        if self.frame_size.0 > 0 && self.frame_size.1 > 0 && self.frame_size.1 < self.frame_size.0 {
            return errors::parse_error("inconsistent frame size, max frame size < min frame size");
        }

        // read sample rate [20 bits]
        let sample_rate_msb = self.reader.read_be_u16()?;
        let sample_rate_lsb = self.reader.read_u8()?;

        // Make the value from the first 16 bits, and then the
        // 4 most significant bits of the next byte
        let sample_rate = (sample_rate_msb as u32) << 4 | (sample_rate_lsb as u32) >> 4;
        if sample_rate == 0 || sample_rate > 655350 {
            return errors::parse_error("sampling rate must be less than 655350");
        }

        // no of channels [3 bits]
        let no_channels = ((sample_rate_lsb >> 1) & 0b0000_0111) + 1;
        if !(1..=8).contains(&no_channels) {
            return errors::parse_error("number of channels must be between 1 and 8");
        }
        let channel_layout = num_channels_to_channel_layout(no_channels);

        // read bits per sample [5 bits]
        let bps_bits = self.reader.read_u8()?;
        let bits_per_sample = ((sample_rate_lsb & 1) << 4 | bps_bits >> 4) + 1;

        // read total samples [36 bits]
        //
        // 'Frames' means inter-channel sample, i.e. one second of 44.1Khz audio will have 44100 samples
        // regardless of the number of channels
        let total_frames =
            ((bps_bits & 0b0000_1111) as u64) << 32 | (self.reader.read_be_u32()? as u64);

        // read md5 signature [128 bits or 16 bytes]
        self.reader.read_into(&mut self.md5)?;

        Ok(audio::AudioInfo {
            codec_type: codecs::CodecType::CODEC_TYPE_FLAC,
            sample_rate,
            total_samples: total_frames * no_channels as u64,
            bits_per_sample: bits_per_sample as u32,
            channels: channel_layout.into_channels(),
            channel_layout,
        })
    }
}

impl AudioReader for FlacReader {
    fn read_header(&mut self) -> Result<audio::AudioInfo> {
        if FLAC_MARKER != &(self.reader.read_bytes(4)?)[..] {
            return errors::parse_error("no fLaC tag Found");
        }

        let mut is_last = false;
        let mut info = errors::parse_error::<audio::AudioInfo>("no stream_info block found");

        while !is_last {
            let header_byte = self.reader.read_u8()?;

            // The first bit specifies whether this is the last block and
            // next 7 bits specify the type of the metadata block
            // https://xiph.org/flac/format.html#metadata_block_header
            is_last = (header_byte >> 7) == 1;
            let block_type = header_byte & 0x7f;
            let metadata_length = self.reader.read_be_u24()?;

            match block_type {
                0 => info = self.read_stream_info(metadata_length),
                127 => info = errors::parse_error("invalid metadata block"),
                _ => self.reader.skip_bytes(metadata_length as usize)?,
            }
        }

        info
    }

    fn buffer(&mut self) -> &mut AudioInputStream {
        &mut self.reader
    }
}

fn num_channels_to_channel_layout(channels: u8) -> audio::ChannelLayout {
    match channels {
        1 => audio::ChannelLayout::Mono,
        2 => audio::ChannelLayout::Stereo,
        3 => audio::ChannelLayout::ThreePointZero,
        4 => audio::ChannelLayout::Quad,
        5 => audio::ChannelLayout::FivePointZero,
        6 => audio::ChannelLayout::FivePointOne,
        7 => audio::ChannelLayout::SixPointOneBack,
        8 => audio::ChannelLayout::SevenPointOne,
        _ => unreachable!(),
    }
}

pub struct FlacSamplesIterator<'r, S: Sample + 'r> {
    reader: &'r mut Box<dyn AudioReader + 'static>,
    audio_info: &'r audio::AudioInfo,
    current_block: frame::Block,
    samples_read: u32,
    current_channel: u32,
    has_failed: bool,
    // flag is set when decoder fails anywhere and buffer should return None
    phantom: std::marker::PhantomData<S>,
}

impl<'r, S: Sample + 'r> FlacSamplesIterator<'r, S> {
    pub fn new(
        reader: &'r mut Box<dyn AudioReader + 'static>,
        info: &'r audio::AudioInfo,
    ) -> Box<dyn AudioSamplesIterator<S> + 'r> {
        Box::new(FlacSamplesIterator::<S> {
            reader,
            audio_info: info,
            current_block: frame::Block::empty(),
            samples_read: 0,
            current_channel: 0,
            has_failed: false,
            phantom: std::marker::PhantomData,
        })
    }
}

impl<'r, S: Sample> AudioSamplesIterator<S> for FlacSamplesIterator<'r, S> {
    fn next(&mut self) -> Option<Result<S>> {
        if self.has_failed {
            return None;
        }

        self.current_channel += 1;

        if self.current_channel >= self.current_block.num_channels() {
            self.current_channel = 0;
            self.samples_read += 1;

            // we read last sample, decode next block
            if self.samples_read >= self.current_block.total_samples() {
                self.samples_read = 0;

                // Replace the current block with an empty one so that we may
                // reuse the current buffer to decode again.
                let current_block =
                    std::mem::replace(&mut self.current_block, frame::Block::empty());

                match frame::decode_next_frame(
                    self.reader.buffer(),
                    current_block.into_buffer(),
                    self.audio_info,
                ) {
                    Some(Ok(next_block)) => {
                        self.current_block = next_block;
                    }
                    Some(Err(error)) => {
                        self.has_failed = true;
                        return Some(Err(error));
                    }
                    _ => {
                        return None;
                    }
                }
            }
        }

        // else just return next sample
        Some(Sample::from_i32(
            self.current_block
                .get_sample(self.current_channel, self.samples_read),
            self.current_block.bits_per_sample(),
        ))
    }
}
