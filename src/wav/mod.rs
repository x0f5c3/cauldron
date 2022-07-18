mod chunks;

use super::io::{AudioInputStream, AudioReader, AudioSamplesIterator, ReadBuffer, Sample};
use super::{audio, errors, Result};

use chunks::*;

const RIFF_MARKER: &[u8; 4] = b"RIFF";
const WAVE_MARKER: &[u8; 4] = b"WAVE";

pub struct WavReader {
    reader: AudioInputStream,
}

impl WavReader {
    pub fn new(reader: AudioInputStream) -> Result<Box<Self>> {
        Ok(Box::new(WavReader { reader }))
    }
}

impl AudioReader for WavReader {
    fn read_header(&mut self) -> Result<audio::AudioInfo> {
        // WAVE file starts with the four bytes 'RIFF' and a file length.
        if RIFF_MARKER != &(self.reader.read_bytes(4)?)[..] {
            return errors::parse_error("no RIFF tag Found");
        }
        let _chunk_size = self.reader.read_le_u32()?;

        // Next four bytes indicate the file type, which should be WAVE.
        if WAVE_MARKER != &(self.reader.read_bytes(4)?)[..] {
            return errors::parse_error("no WAVE tag found");
        }

        // read until data chunk to get full info
        let mut info: Option<audio::AudioInfo> = None;
        while let Some(chunk) = read_next_chunk(&mut self.reader)? {
            if let Chunk::Fmt(audio_info) = chunk {
                info = Some(audio_info);
            } else if let Chunk::Data(data_len) = chunk {
                if let Some(mut inf) = info {
                    inf.total_samples = (data_len / (inf.bits_per_sample / 8)) as u64;
                    return Ok(inf);
                }
            }
        }
        errors::parse_error("no 'fmt' chunk found")
    }

    fn buffer(&mut self) -> &mut AudioInputStream {
        &mut self.reader
    }
}

pub struct WavSamplesIterator<'r, S: Sample> {
    reader: &'r mut Box<dyn AudioReader + 'static>,
    audio_info: &'r audio::AudioInfo,
    samples_left: u64,
    phantom: std::marker::PhantomData<S>,
}

impl<'r, S: Sample + 'r> WavSamplesIterator<'r, S> {
    pub fn new(
        reader: &'r mut Box<dyn AudioReader + 'static>,
        info: &'r audio::AudioInfo,
    ) -> Box<Self> {
        Box::new(WavSamplesIterator {
            reader,
            audio_info: info,
            samples_left: info.total_samples,
            phantom: std::marker::PhantomData,
        })
    }
}

impl<'r, S: Sample> AudioSamplesIterator<S> for WavSamplesIterator<'r, S> {
    fn next(&mut self) -> Option<Result<S>> {
        if self.samples_left > 0 {
            let sample = Sample::read_pcm(&mut self.reader.buffer(), self.audio_info.codec_type);
            self.samples_left -= 1;
            return Some(sample);
        }

        None
    }
}
