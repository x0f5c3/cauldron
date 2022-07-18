mod dynamic_buf_reader;
mod read;
mod write;

use std::io;

use super::codecs::CodecType;
use super::{audio, errors, utils, Result};

pub use dynamic_buf_reader::DynamicBufReader;
pub use read::{BitStream, ReadBuffer};
pub use write::WriteBuffer;

pub type AudioInputStream = DynamicBufReader<Box<dyn io::Read + Send>>;

pub trait IntoAudioInputStream {
    fn into_stream(self) -> Result<AudioInputStream>;
}

impl IntoAudioInputStream for String {
    fn into_stream(self) -> Result<AudioInputStream> {
        let file = std::fs::File::open(self)?;
        Ok(AudioInputStream::new(Box::new(file)))
    }
}

impl IntoAudioInputStream for &str {
    fn into_stream(self) -> Result<AudioInputStream> {
        let file = std::fs::File::open(self)?;
        Ok(AudioInputStream::new(Box::new(file)))
    }
}

impl IntoAudioInputStream for &std::path::Path {
    fn into_stream(self) -> Result<AudioInputStream> {
        let file = std::fs::File::open(self)?;
        Ok(AudioInputStream::new(Box::new(file)))
    }
}

/// A type that can be used to represent audio samples.
///
/// It makes decoding can be generic over `u8`, `i16`, `i32` and `f32`.
///
/// All integer formats with bit depths up to 32 bits per sample can be decoded
/// into `i32`, but it takes up more memory. If you know beforehand that you
/// will be reading a file with 16 bits per sample, then decoding into an `i16`
/// will be sufficient.
pub trait Sample: Sized + Copy + Send {
    /// Reads the audio sample from the data buffer
    fn read_pcm<R: ReadBuffer>(reader: &mut R, codec: CodecType) -> Result<Self>;

    /// Writes the audio sample to the data buffer
    fn write_pcm<W: WriteBuffer>(self, writer: &mut W, bits: u16) -> Result<()>;

    fn from_i32(value: i32, bits: u32) -> Result<Self>;

    fn from_f32(value: f32) -> Result<Self>;
}

impl Sample for u8 {
    #[inline(always)]
    fn read_pcm<R: ReadBuffer>(reader: &mut R, codec: CodecType) -> Result<u8> {
        match codec {
            CodecType::CODEC_TYPE_PCM_U8 => Ok(reader.read_u8()?),
            _ => errors::unsupported_error("unsupported for u8"),
        }
    }

    fn write_pcm<W: WriteBuffer>(self, writer: &mut W, bits: u16) -> Result<()> {
        match bits {
            8 => Ok(writer.write_u8(self)?),
            16 => Ok(writer.write_le_i16(self as i16)?),
            24 => Ok(writer.write_le_i24(self as i32)?),
            32 => Ok(writer.write_le_i32(self as i32)?),
            _ => errors::unsupported_error(""),
        }
    }

    #[inline(always)]
    fn from_i32(value: i32, bits: u32) -> Result<u8> {
        if bits <= 8 {
            Ok(value as u8)
        } else {
            errors::unsupported_error("invalid target for bits per sample")
        }
    }

    #[inline(always)]
    fn from_f32(_value: f32) -> Result<u8> {
        errors::unsupported_error("unsupported sample format")
    }
}

impl Sample for i16 {
    #[inline(always)]
    fn read_pcm<R: ReadBuffer>(reader: &mut R, codec: CodecType) -> Result<i16> {
        match codec {
            CodecType::CODEC_TYPE_PCM_U8 => Ok(reader.read_u8().map(|x| x as i16)?),
            CodecType::CODEC_TYPE_PCM_S16LE => Ok(reader.read_le_i16()?),
            _ => errors::unsupported_error("unsupported for i16"),
        }
    }

    fn write_pcm<W: WriteBuffer>(self, writer: &mut W, bits: u16) -> Result<()> {
        match bits {
            8 => Ok(writer.write_u8(utils::u8_from_signed(utils::narrow_to_i8(self as i32)?))?),
            16 => Ok(writer.write_le_i16(self)?),
            24 => Ok(writer.write_le_i24(self as i32)?),
            32 => Ok(writer.write_le_i32(self as i32)?),
            _ => errors::unsupported_error(""),
        }
    }

    #[inline(always)]
    fn from_i32(value: i32, bits: u32) -> Result<i16> {
        if bits <= 16 {
            Ok(value as i16)
        } else {
            errors::unsupported_error("invalid target for bits per sample")
        }
    }

    #[inline(always)]
    fn from_f32(_value: f32) -> Result<i16> {
        errors::unsupported_error("unsupported sample format")
    }
}

impl Sample for i32 {
    #[inline(always)]
    fn read_pcm<R: ReadBuffer>(reader: &mut R, codec: CodecType) -> Result<i32> {
        match codec {
            CodecType::CODEC_TYPE_PCM_U8 => Ok(reader.read_u8().map(|x| x as i32)?),
            CodecType::CODEC_TYPE_PCM_S16LE => Ok(reader.read_le_i16().map(|x| x as i32)?),
            CodecType::CODEC_TYPE_PCM_S24LE => Ok(reader.read_le_i24()?),
            CodecType::CODEC_TYPE_PCM_S32LE => Ok(reader.read_le_i32()?),
            _ => errors::unsupported_error("unsupported for i32"),
        }
    }

    fn write_pcm<W: WriteBuffer>(self, writer: &mut W, bits: u16) -> Result<()> {
        match bits {
            8 => Ok(writer.write_u8(utils::u8_from_signed(utils::narrow_to_i8(self as i32)?))?),
            16 => Ok(writer.write_le_i16(utils::narrow_to_i16(self)?)?),
            24 => Ok(writer.write_le_i24(utils::narrow_to_i24(self)?)?),
            32 => Ok(writer.write_le_i32(self as i32)?),
            _ => errors::unsupported_error::<()>(""),
        }
    }

    #[inline(always)]
    fn from_i32(value: i32, _bits: u32) -> Result<i32> {
        Ok(value)
    }

    #[inline(always)]
    fn from_f32(_value: f32) -> Result<i32> {
        errors::unsupported_error("unsupported sample format")
    }
}

impl Sample for f32 {
    #[inline(always)]
    fn read_pcm<R: ReadBuffer>(reader: &mut R, codec: CodecType) -> Result<f32> {
        match codec {
            CodecType::CODEC_TYPE_PCM_U8 => Ok(reader.read_u8().map(|x| x as f32 / 255.0)?),
            CodecType::CODEC_TYPE_PCM_S16LE => Ok(reader.read_le_i16()? as f32 / 32_768.0),
            CodecType::CODEC_TYPE_PCM_S24LE => Ok(reader.read_le_i24()? as f32 / 2_147_483_648.0),
            CodecType::CODEC_TYPE_PCM_S32LE => Ok(reader.read_le_i32()? as f32 / 2_147_483_648.0),
            CodecType::CODEC_TYPE_PCM_F32LE => Ok(reader.read_le_f32()?),
            _ => errors::unsupported_error("unsupported for f32"),
        }
    }

    fn write_pcm<W: WriteBuffer>(self, writer: &mut W, bits: u16) -> Result<()> {
        match bits {
            32 => Ok(writer.write_le_f32(self)?),
            _ => errors::unsupported_error::<()>(""),
        }
    }

    #[inline(always)]
    fn from_i32(value: i32, bits: u32) -> Result<f32> {
        match bits {
            16 => Ok(value as f32 / 32_768.0),
            24 => Ok(value as f32 / 2_147_483_648.0),
            32 => Ok(value as f32 / 2_147_483_648.0),
            _ => errors::unsupported_error("unsupported bits per sample for f32"),
        }
    }

    #[inline(always)]
    fn from_f32(value: f32) -> Result<f32> {
        Ok(value)
    }
}

impl Sample for f64 {
    #[inline(always)]
    fn read_pcm<R: ReadBuffer>(reader: &mut R, codec: CodecType) -> Result<Self> {
        match codec {
            CodecType::CODEC_TYPE_PCM_U8 => Ok(reader.read_u8().map(|x| x as f64 / 255.0)?),
            CodecType::CODEC_TYPE_PCM_S16LE => Ok(reader.read_le_i16()? as f64 / 32_768.0),
            CodecType::CODEC_TYPE_PCM_S24LE => Ok(reader.read_le_i24()? as f64 / 2_147_483_648.0),
            CodecType::CODEC_TYPE_PCM_S32LE => Ok(reader.read_le_i32()? as f64 / 2_147_483_648.0),
            CodecType::CODEC_TYPE_PCM_F32LE => Ok(reader.read_le_f32()? as f64 / f32::MAX as f64),
            CodecType::CODEC_TYPE_PCM_F64LE => Ok(reader.read_le_f64()?),
            _ => errors::unsupported_error("unsupported for f32"),
        }
    }

    #[inline(always)]
    fn write_pcm<W: WriteBuffer>(self, writer: &mut W, bits: u16) -> Result<()> {
        match bits {
            64 => Ok(writer.write_le_f64(self)?),
            _ => errors::unsupported_error::<()>(""),
        }
    }

    #[inline(always)]
    fn from_i32(value: i32, bits: u32) -> Result<Self> {
        match bits {
            16 => Ok(value as f64 / 32_768.0),
            24 => Ok(value as f64 / 2_147_483_648.0),
            32 => Ok(value as f64 / 2_147_483_648.0),
            64 => Ok(value as f64 / (i64::MAX as f64 + 1.0)),
            _ => errors::unsupported_error("unsupported bits per sample for f32"),
        }
    }

    #[inline(always)]
    fn from_f32(value: f32) -> Result<Self> {
        Ok(value.into())
    }
}

/// A `AudioReader` is a container demuxer. It provides methods to probe a media container for
/// information and access the streams encapsulated in the container.
pub trait AudioReader: Send {
    /// Reads the header and initializes audio info
    fn read_header(&mut self) -> Result<audio::AudioInfo>;

    /// Returns the buffer for the iterator
    fn buffer(&mut self) -> &mut AudioInputStream;
}

/// Returns a lazy iterator on audio samples
pub trait AudioSamplesIterator<S: Sample>: Send {
    fn next(&mut self) -> Option<Result<S>>;
}

impl<'r, S: Sample> Iterator for dyn AudioSamplesIterator<S> + 'r {
    type Item = Result<S>;

    fn next(&mut self) -> Option<Result<S>> {
        self.next()
    }
}
