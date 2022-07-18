//! The `codecs` module defines format flags and codecs.

use std::fmt;

/// Format flag to specify when reading audio
#[derive(PartialEq, Eq, Debug)]
pub enum FormatFlag {
    /// aac
    AAC = 0,
    /// flac
    FLAC = 1,
    /// mp3 - mpeg layer 3
    MP3 = 2,
    /// raw audio
    PCM = 3,
    /// wave audio
    WAV = 4,
    /// vorbis or ogg
    VORBIS = 5,
}

impl fmt::Display for FormatFlag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A `CodecType` is a unique identifier used to identify a specific codec.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum CodecType {
    /// Null decoder, simply discards all data.
    CODEC_TYPE_NULL = 0,

    // PCM codecs
    //-----------
    /// PCM signed 32-bit little-endian interleaved
    CODEC_TYPE_PCM_S32LE,
    /// PCM signed 32-bit little-endian planar
    CODEC_TYPE_PCM_S32LE_PLANAR,
    /// PCM signed 32-bit big-endian interleaved
    CODEC_TYPE_PCM_S32BE,
    /// PCM signed 32-bit big-endian planar
    CODEC_TYPE_PCM_S32BE_PLANAR,
    /// PCM signed 24-bit little-endian interleaved
    CODEC_TYPE_PCM_S24LE,
    /// PCM signed 24-bit little-endian planar
    CODEC_TYPE_PCM_S24LE_PLANAR,
    /// PCM signed 24-bit big-endian interleaved
    CODEC_TYPE_PCM_S24BE,
    /// PCM signed 24-bit big-endian planar
    CODEC_TYPE_PCM_S24BE_PLANAR,
    /// PCM signed 16-bit little-endian interleaved
    CODEC_TYPE_PCM_S16LE,
    /// PCM signed 16-bit little-endian planar
    CODEC_TYPE_PCM_S16LE_PLANAR,
    /// PCM signed 16-bit big-endian interleaved
    CODEC_TYPE_PCM_S16BE,
    /// PCM signed 16-bit big-endian planar
    CODEC_TYPE_PCM_S16BE_PLANAR,
    /// PCM signed 8-bit interleaved
    CODEC_TYPE_PCM_S8,
    /// PCM signed 8-bit planar
    CODEC_TYPE_PCM_S8_PLANAR,
    /// PCM unsigned 32-bit little-endian interleaved
    CODEC_TYPE_PCM_U32LE,
    /// PCM unsigned 32-bit little-endian planar
    CODEC_TYPE_PCM_U32LE_PLANAR,
    /// PCM unsigned 32-bit big-endian interleaved
    CODEC_TYPE_PCM_U32BE,
    /// PCM unsigned 32-bit big-endian planar
    CODEC_TYPE_PCM_U32BE_PLANAR,
    /// PCM unsigned 24-bit little-endian interleaved
    CODEC_TYPE_PCM_U24LE,
    /// PCM unsigned 24-bit little-endian planar
    CODEC_TYPE_PCM_U24LE_PLANAR,
    /// PCM unsigned 24-bit big-endian interleaved
    CODEC_TYPE_PCM_U24BE,
    /// PCM unsigned 24-bit big-endian planar
    CODEC_TYPE_PCM_U24BE_PLANAR,
    /// PCM unsigned 16-bit little-endian interleaved
    CODEC_TYPE_PCM_U16LE,
    /// PCM unsigned 16-bit little-endian planar
    CODEC_TYPE_PCM_U16LE_PLANAR,
    /// PCM unsigned 16-bit big-endian interleaved
    CODEC_TYPE_PCM_U16BE,
    /// PCM unsigned 16-bit big-endian planar
    CODEC_TYPE_PCM_U16BE_PLANAR,
    /// PCM unsigned 8-bit interleaved
    CODEC_TYPE_PCM_U8,
    /// PCM unsigned 8-bit planar
    CODEC_TYPE_PCM_U8_PLANAR,
    /// PCM 32-bit little-endian floating point interleaved
    CODEC_TYPE_PCM_F32LE,
    /// PCM 32-bit little-endian floating point planar
    CODEC_TYPE_PCM_F32LE_PLANAR,
    /// PCM 32-bit big-endian floating point interleaved
    CODEC_TYPE_PCM_F32BE,
    /// PCM 32-bit big-endian floating point planar
    CODEC_TYPE_PCM_F32BE_PLANAR,
    /// PCM 64-bit little-endian floating point interleaved
    CODEC_TYPE_PCM_F64LE,
    /// PCM 64-bit little-endian floating point planar
    CODEC_TYPE_PCM_F64LE_PLANAR,
    /// PCM 64-bit big-endian floating point interleaved
    CODEC_TYPE_PCM_F64BE,
    /// PCM 64-bit big-endian floating point planar
    CODEC_TYPE_PCM_F64BE_PLANAR,
    /// PCM A-law
    CODEC_TYPE_PCM_ALAW,
    /// PCM Mu-law
    CODEC_TYPE_PCM_MULAW,

    // Compressed audio codecs
    //------------------------
    /// Free Lossless Audio Codec (FLAC)
    CODEC_TYPE_FLAC,
    /// MPEG Layer 3 MP3
    CODEC_TYPE_MP3,
    /// Advanced Audio Coding (AAC)
    CODEC_TYPE_AAC,
    /// Vorbis
    CODEC_TYPE_VORBIS,
}

/// convert codec type to string
pub fn codec_to_str(codec_type: &CodecType) -> &str {
    match codec_type {
        CodecType::CODEC_TYPE_PCM_S32LE => "pcm_s32le",
        CodecType::CODEC_TYPE_PCM_S32LE_PLANAR => "pcm_s32le_planar",
        CodecType::CODEC_TYPE_PCM_S32BE => "pcm_s32be",
        CodecType::CODEC_TYPE_PCM_S32BE_PLANAR => "pcm_s32be_planar",
        CodecType::CODEC_TYPE_PCM_S24LE => "pcm_s24le",
        CodecType::CODEC_TYPE_PCM_S24LE_PLANAR => "pcm_s24le_planar",
        CodecType::CODEC_TYPE_PCM_S24BE => "pcm_s24be",
        CodecType::CODEC_TYPE_PCM_S24BE_PLANAR => "pcm_s24be_planar",
        CodecType::CODEC_TYPE_PCM_S16LE => "pcm_s16le",
        CodecType::CODEC_TYPE_PCM_S16LE_PLANAR => "pcm_s16le_planar",
        CodecType::CODEC_TYPE_PCM_S16BE => "pcm_s16be",
        CodecType::CODEC_TYPE_PCM_S16BE_PLANAR => "pcm_s16be_planar",
        CodecType::CODEC_TYPE_PCM_S8 => "pcm_s8",
        CodecType::CODEC_TYPE_PCM_S8_PLANAR => "pcm_s8_planar",
        CodecType::CODEC_TYPE_PCM_U32LE => "pcm_u32le",
        CodecType::CODEC_TYPE_PCM_U32LE_PLANAR => "pcm_u32le_planar",
        CodecType::CODEC_TYPE_PCM_U32BE => "pcm_u32be",
        CodecType::CODEC_TYPE_PCM_U32BE_PLANAR => "pcm_u32be_planar",
        CodecType::CODEC_TYPE_PCM_U24LE => "pcm_u24le",
        CodecType::CODEC_TYPE_PCM_U24LE_PLANAR => "pcm_u24le_planar",
        CodecType::CODEC_TYPE_PCM_U24BE => "pcm_u24be",
        CodecType::CODEC_TYPE_PCM_U24BE_PLANAR => "pcm_u24be_planar",
        CodecType::CODEC_TYPE_PCM_U16LE => "pcm_u16le",
        CodecType::CODEC_TYPE_PCM_U16LE_PLANAR => "pcm_u16le_planar",
        CodecType::CODEC_TYPE_PCM_U16BE => "pcm_u16be",
        CodecType::CODEC_TYPE_PCM_U16BE_PLANAR => "pcm_u16be_planar",
        CodecType::CODEC_TYPE_PCM_U8 => "pcm_u8",
        CodecType::CODEC_TYPE_PCM_U8_PLANAR => "pcm_u8_planar",
        CodecType::CODEC_TYPE_PCM_F32LE => "pcm_f32le",
        CodecType::CODEC_TYPE_PCM_F32LE_PLANAR => "pcm_f32le_planar",
        CodecType::CODEC_TYPE_PCM_F32BE => "pcm_f32be",
        CodecType::CODEC_TYPE_PCM_F32BE_PLANAR => "pcm_f32be_planar",
        CodecType::CODEC_TYPE_PCM_F64LE => "pcm_f64le",
        CodecType::CODEC_TYPE_PCM_F64LE_PLANAR => "pcm_f64le_planar",
        CodecType::CODEC_TYPE_PCM_F64BE => "pcm_f64be",
        CodecType::CODEC_TYPE_PCM_F64BE_PLANAR => "pcm_f64be_planar",
        CodecType::CODEC_TYPE_PCM_ALAW => "pcm_alaw",
        CodecType::CODEC_TYPE_PCM_MULAW => "pcm_mulaw",
        CodecType::CODEC_TYPE_FLAC => "flac",
        CodecType::CODEC_TYPE_MP3 => "mp3",
        CodecType::CODEC_TYPE_AAC => "aac",
        CodecType::CODEC_TYPE_VORBIS => "vorbis",
        CodecType::CODEC_TYPE_NULL => "unknown",
    }
}

impl fmt::Display for CodecType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", codec_to_str(self))
    }
}
