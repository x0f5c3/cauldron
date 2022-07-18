mod frame;
mod types;

use super::io::{AudioInputStream, AudioReader, AudioSamplesIterator, Sample};
use super::{audio, codecs, Result};

pub struct Mp3Reader {
    reader: AudioInputStream,
}

impl Mp3Reader {
    pub fn new(reader: AudioInputStream) -> Result<Box<Self>> {
        Ok(Box::new(Mp3Reader { reader }))
    }
}

impl AudioReader for Mp3Reader {
    fn read_header(&mut self) -> Result<audio::AudioInfo> {
        Ok(audio::AudioInfo {
            codec_type: codecs::CodecType::CODEC_TYPE_MP3,
            sample_rate: 0,
            total_samples: 0,
            bits_per_sample: 0,
            channels: audio::ChannelLayout::Mono.into_channels(),
            channel_layout: audio::ChannelLayout::Mono,
        })
    }

    fn buffer(&mut self) -> &mut AudioInputStream {
        &mut self.reader
    }
}

pub struct Mp3SamplesIterator<'r, S: Sample + 'r> {
    reader: &'r mut Box<dyn AudioReader + 'static>,
    _audio_info: &'r audio::AudioInfo,
    phantom: std::marker::PhantomData<S>,
    current_block: frame::Block,
    decoder_state: frame::DecoderState,
    samples_read: u32,
    current_channel: u32,
    has_failed: bool,
}

impl<'r, S: Sample + 'r> Mp3SamplesIterator<'r, S> {
    pub fn new(
        reader: &'r mut Box<dyn AudioReader + 'static>,
        info: &'r audio::AudioInfo,
    ) -> Box<Self> {
        Box::new(Mp3SamplesIterator::<S> {
            reader,
            _audio_info: info,
            phantom: std::marker::PhantomData,
            current_block: frame::Block::empty(),
            decoder_state: frame::DecoderState::new(),
            samples_read: 0,
            current_channel: 0,
            has_failed: false,
        })
    }
}

impl<'r, S: Sample> AudioSamplesIterator<S> for Mp3SamplesIterator<'r, S> {
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

                let current_block =
                    std::mem::replace(&mut self.current_block, frame::Block::empty());

                match frame::decode_next_frame::<AudioInputStream>(
                    self.reader.buffer(),
                    &mut self.decoder_state,
                    current_block.into_buffer(),
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
        Some(Sample::from_f32(
            self.current_block
                .get_sample(self.current_channel, self.samples_read),
        ))
    }
}
