use crate::audio::{AudioInfo, ChannelLayout, Channels};
use crate::io::ReadBuffer;
use crate::{codecs, errors, Result};

/// A chunk in a Riff Wave file.
pub enum Chunk {
    /// format chunk, fully parsed into a AudioInfo
    Fmt(AudioInfo),
    /// data chunk, where the samples are actually stored
    Data(u32),
    /// any other riff chunk
    Unknown([u8; 4], u32),
}

// The different compression format definitions can be found in mmreg.h that is
// part of the Windows SDK.
const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_ALAW: u16 = 0x0006;
const WAVE_FORMAT_MULAW: u16 = 0x0007;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xfffe;

// These GUIDs identify the format of the data chunks.
// https://docs.microsoft.com/en-us/windows-hardware/drivers/audio/subformat-guids-for-compressed-audio-formats
const KSDATAFORMAT_SUBTYPE_PCM: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: [u8; 16] = [
    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const KSDATAFORMAT_SUBTYPE_ALAW: [u8; 16] = [
    0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const KSDATAFORMAT_SUBTYPE_MULAW: [u8; 16] = [
    0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];

/// Parse the next chunk from the reader.
///
/// Returns None at end of file, or a `Chunk` instance depending on the chunk kind.
pub fn read_next_chunk<R: ReadBuffer>(reader: &mut R) -> Result<Option<Chunk>> {
    let mut chunk_type = [0; 4];
    // check for EOF
    if reader.read_into(&mut chunk_type).is_err() {
        return Ok(None);
    }
    // length of chunk bytes excluding chunk id and itself
    // For chunks we don't want to handle we will just skip these many bytes
    let len = reader.read_le_u32()?;

    match &chunk_type {
        b"fmt " => {
            let info = read_fmt_chunk(reader, len);
            Ok(Some(Chunk::Fmt(info?)))
        }
        b"data" => Ok(Some(Chunk::Data(len))),
        _ => {
            reader.skip_bytes(len as usize)?;
            Ok(Some(Chunk::Unknown(chunk_type, len)))
        }
    }
}

/// Reads the fmt chunk of the file, returns the information it provides.
fn read_fmt_chunk<R: ReadBuffer>(reader: &mut R, chunk_len: u32) -> Result<AudioInfo> {
    // A minimum chunk length of at least 16 is assumed.
    // https://sites.google.com/site/musicgapi/technical-documents/wav-file-format#fmt
    if chunk_len < 16 {
        return errors::parse_error("invalid fmt chunk size");
    }

    let format_tag = reader.read_le_u16()?; // type of codec
    let n_channels = reader.read_le_u16()?;
    let sample_rate = reader.read_le_u32()?;
    let n_bytes_per_sec = reader.read_le_u32()?;
    let block_align = reader.read_le_u16()?;
    let bits_per_sample = reader.read_le_u16()?;

    if n_channels == 0 {
        return errors::parse_error("number channels is 0");
    }

    // Two of the stored fields are redundant, and may be ignored. We do
    // validate them to fail early for ill-formed files.
    //
    // BlockAlign = SignificantBitsPerSample / 8 * NumChannels
    // AvgBytesPerSec = SampleRate * BlockAlign
    if (Some(bits_per_sample) != (block_align / n_channels).checked_mul(8))
        || (Some(n_bytes_per_sec) != (block_align as u32).checked_mul(sample_rate))
    {
        return errors::parse_error("inconsistent fmt chunk");
    }

    let audio_info = AudioInfo {
        codec_type: codecs::CodecType::CODEC_TYPE_NULL,
        sample_rate,
        total_samples: 0,
        bits_per_sample: bits_per_sample as u32,
        channels: Channels::FRONT_LEFT,
        channel_layout: ChannelLayout::Mono,
    };

    match format_tag {
        WAVE_FORMAT_PCM => read_wave_format_pcm(reader, chunk_len, n_channels, audio_info),
        WAVE_FORMAT_IEEE_FLOAT => read_wave_format_ieee(reader, chunk_len, n_channels, audio_info),
        WAVE_FORMAT_ALAW => read_wave_format_alaw(reader, chunk_len, n_channels, audio_info),
        WAVE_FORMAT_MULAW => read_wave_format_mulaw(reader, chunk_len, n_channels, audio_info),
        WAVE_FORMAT_EXTENSIBLE => read_wave_format_ext(reader, chunk_len, audio_info),
        _ => errors::unsupported_error("encoding format not supported"),
    }
}

fn read_wave_format_pcm<R: ReadBuffer>(
    reader: &mut R,
    chunk_len: u32,
    n_channels: u16,
    mut audio_info: AudioInfo,
) -> Result<AudioInfo> {
    // It means extra data has been added to pcm format which is unnecessary
    // So, just read the bytes and ignore them
    if chunk_len > 16 {
        reader.skip_bytes((chunk_len - 16) as usize)?;
    }

    // for pcm data is always interleaved little endian
    audio_info.codec_type = match audio_info.bits_per_sample {
        8 => codecs::CodecType::CODEC_TYPE_PCM_U8,
        16 => codecs::CodecType::CODEC_TYPE_PCM_S16LE,
        24 => codecs::CodecType::CODEC_TYPE_PCM_S24LE,
        32 => codecs::CodecType::CODEC_TYPE_PCM_S32LE,
        _ => {
            return errors::parse_error("Bits per sample for fmt_pcm must be 8, 16, 24 or 32 bits.")
        }
    };

    // The PCM format only supports max 2 channels.
    audio_info.channel_layout = match n_channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        _ => return errors::parse_error("Only max two channels supported for fmt_pcm."),
    };
    audio_info.channels = ChannelLayout::into_channels(audio_info.channel_layout);

    Ok(audio_info)
}

fn read_wave_format_ieee<R: ReadBuffer>(
    reader: &mut R,
    chunk_len: u32,
    n_channels: u16,
    mut audio_info: AudioInfo,
) -> Result<AudioInfo> {
    // WaveFormat for a IEEE format should not be extended, but it extra data length
    // parameter may be there.
    let mut extra_size = 0;
    if chunk_len == 18 {
        extra_size = reader.read_le_u16()?;
    }
    if extra_size != 0 || chunk_len > 18 {
        return errors::parse_error("Malformed fmt_ieee chunk.");
    }

    // only 32bit float supported for wav
    audio_info.codec_type = match audio_info.bits_per_sample {
        32 => codecs::CodecType::CODEC_TYPE_PCM_F32LE,
        64 => codecs::CodecType::CODEC_TYPE_PCM_F64LE,
        _ => return errors::parse_error("Bits per sample for fmt_ieee must be 32 or 64 bits."),
    };

    // IEEE format only supports max 2 channels.
    audio_info.channel_layout = match n_channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        _ => return errors::parse_error("Max two channels supported for fmt_ieee."),
    };
    audio_info.channels = ChannelLayout::into_channels(audio_info.channel_layout);

    Ok(audio_info)
}

fn read_wave_format_ext<R: ReadBuffer>(
    reader: &mut R,
    chunk_len: u32,
    mut audio_info: AudioInfo,
) -> Result<AudioInfo> {
    // https://docs.microsoft.com/en-us/windows-hardware/drivers/audio/extensible-wave-format-descriptors
    // For extensible Wave format 40 bytes of data should be present
    if chunk_len < 40 {
        return errors::parse_error("Malformed fmt_ext chunk.");
    }

    // The size of the extra data for the Extensible format should be exactly 22 bytes.
    let extra_size = reader.read_le_u16()?;
    if extra_size != 22 {
        return errors::parse_error("Extra data size not 22 bytes for fmt_ext chunk.");
    }

    if (audio_info.bits_per_sample & 0x7) != 0 {
        return errors::parse_error(
            "Bits per encoded sample for fmt_ext must be a multiple of 8 bits.",
        );
    }
    audio_info.bits_per_sample = reader.read_le_u16()? as u32;

    let channel_mask = reader.read_le_u32()?;
    let mut sub_format_guid = [0u8; 16];
    reader.read_into(&mut sub_format_guid)?;

    audio_info.codec_type = match sub_format_guid {
        KSDATAFORMAT_SUBTYPE_PCM => {
            // Only support up-to 32-bit integer samples.
            if audio_info.bits_per_sample > 32 {
                return errors::parse_error(
                    "Bits per sample for fmt_ext PCM sub-type must be <= 32 bits.",
                );
            }

            // Use bits per coded sample to select the codec to use. If bits per sample is less than the bits per
            // coded sample, the codec will expand the sample during decode.
            match audio_info.bits_per_sample {
                8 => codecs::CodecType::CODEC_TYPE_PCM_U8,
                16 => codecs::CodecType::CODEC_TYPE_PCM_S16LE,
                24 => codecs::CodecType::CODEC_TYPE_PCM_S24LE,
                32 => codecs::CodecType::CODEC_TYPE_PCM_S32LE,
                _ => unreachable!(),
            }
        }
        KSDATAFORMAT_SUBTYPE_IEEE_FLOAT => {
            if audio_info.bits_per_sample == 32 {
                codecs::CodecType::CODEC_TYPE_PCM_F32LE
            } else if audio_info.bits_per_sample == 64 {
                codecs::CodecType::CODEC_TYPE_PCM_F64LE
            } else {
                return errors::parse_error(
                    "Bits per sample for fmt_ext IEEE sub-type must be 32 or 64 bits.",
                );
            }
        }
        KSDATAFORMAT_SUBTYPE_ALAW => codecs::CodecType::CODEC_TYPE_PCM_ALAW,
        KSDATAFORMAT_SUBTYPE_MULAW => codecs::CodecType::CODEC_TYPE_PCM_MULAW,
        _ => return errors::unsupported_error("Unsupported fmt_ext sub-type."),
    };

    audio_info.channels = decode_channel_mask(channel_mask);
    audio_info.channel_layout = match audio_info.channels.count() {
        2 => ChannelLayout::Stereo,
        3 => ChannelLayout::ThreePointZero,
        4 => ChannelLayout::Quad,
        6 => ChannelLayout::FivePointOne,
        8 => ChannelLayout::SevenPointOne,
        _ => ChannelLayout::Mono,
    };

    Ok(audio_info)
}

fn read_wave_format_alaw<R: ReadBuffer>(
    reader: &mut R,
    chunk_len: u32,
    n_channels: u16,
    mut audio_info: AudioInfo,
) -> Result<AudioInfo> {
    if chunk_len > 16 {
        reader.skip_bytes((chunk_len - 16) as usize)?;
    }
    audio_info.codec_type = codecs::CodecType::CODEC_TYPE_PCM_ALAW;
    audio_info.channel_layout = match n_channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        _ => return errors::parse_error("Only max two channels supported for fmt_alaw."),
    };
    audio_info.channels = ChannelLayout::into_channels(audio_info.channel_layout);
    Ok(audio_info)
}

fn read_wave_format_mulaw<R: ReadBuffer>(
    reader: &mut R,
    chunk_len: u32,
    n_channels: u16,
    mut audio_info: AudioInfo,
) -> Result<AudioInfo> {
    if chunk_len > 16 {
        reader.skip_bytes((chunk_len - 16) as usize)?;
    }
    audio_info.codec_type = codecs::CodecType::CODEC_TYPE_PCM_MULAW;
    audio_info.channel_layout = match n_channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        _ => return errors::parse_error("Only max two channels supported for fmt_mulaw."),
    };
    audio_info.channels = ChannelLayout::into_channels(audio_info.channel_layout);

    Ok(audio_info)
}

fn decode_channel_mask(channel_mask: u32) -> Channels {
    const SPEAKER_FRONT_LEFT: u32 = 0x1;
    const SPEAKER_FRONT_RIGHT: u32 = 0x2;
    const SPEAKER_FRONT_CENTER: u32 = 0x4;
    const SPEAKER_LOW_FREQUENCY: u32 = 0x8;
    const SPEAKER_BACK_LEFT: u32 = 0x10;
    const SPEAKER_BACK_RIGHT: u32 = 0x20;
    const SPEAKER_FRONT_LEFT_OF_CENTER: u32 = 0x40;
    const SPEAKER_FRONT_RIGHT_OF_CENTER: u32 = 0x80;
    const SPEAKER_BACK_CENTER: u32 = 0x100;
    const SPEAKER_SIDE_LEFT: u32 = 0x200;
    const SPEAKER_SIDE_RIGHT: u32 = 0x400;
    const SPEAKER_TOP_CENTER: u32 = 0x800;
    const SPEAKER_TOP_FRONT_LEFT: u32 = 0x1000;
    const SPEAKER_TOP_FRONT_CENTER: u32 = 0x2000;
    const SPEAKER_TOP_FRONT_RIGHT: u32 = 0x4000;
    const SPEAKER_TOP_BACK_LEFT: u32 = 0x8000;
    const SPEAKER_TOP_BACK_CENTER: u32 = 0x10000;
    const SPEAKER_TOP_BACK_RIGHT: u32 = 0x20000;

    let mut channels = Channels::empty();

    if channel_mask & SPEAKER_FRONT_LEFT != 0 {
        channels |= Channels::FRONT_LEFT;
    }
    if channel_mask & SPEAKER_FRONT_RIGHT != 0 {
        channels |= Channels::FRONT_RIGHT;
    }
    if channel_mask & SPEAKER_FRONT_CENTER != 0 {
        channels |= Channels::FRONT_CENTRE;
    }
    if channel_mask & SPEAKER_LOW_FREQUENCY != 0 {
        channels |= Channels::LFE1;
    }
    if channel_mask & SPEAKER_BACK_LEFT != 0 {
        channels |= Channels::BACK_LEFT;
    }
    if channel_mask & SPEAKER_BACK_RIGHT != 0 {
        channels |= Channels::BACK_RIGHT;
    }
    if channel_mask & SPEAKER_FRONT_LEFT_OF_CENTER != 0 {
        channels |= Channels::FRONT_LEFT_CENTRE;
    }
    if channel_mask & SPEAKER_FRONT_RIGHT_OF_CENTER != 0 {
        channels |= Channels::FRONT_RIGHT_CENTRE;
    }
    if channel_mask & SPEAKER_BACK_CENTER != 0 {
        channels |= Channels::BACK_CENTRE;
    }
    if channel_mask & SPEAKER_SIDE_LEFT != 0 {
        channels |= Channels::SIDE_LEFT;
    }
    if channel_mask & SPEAKER_SIDE_RIGHT != 0 {
        channels |= Channels::SIDE_RIGHT;
    }
    if channel_mask & SPEAKER_TOP_CENTER != 0 {
        channels |= Channels::TOP_CENTRE;
    }
    if channel_mask & SPEAKER_TOP_FRONT_LEFT != 0 {
        channels |= Channels::TOP_FRONT_LEFT;
    }
    if channel_mask & SPEAKER_TOP_FRONT_CENTER != 0 {
        channels |= Channels::TOP_FRONT_CENTRE;
    }
    if channel_mask & SPEAKER_TOP_FRONT_RIGHT != 0 {
        channels |= Channels::TOP_FRONT_RIGHT;
    }
    if channel_mask & SPEAKER_TOP_BACK_LEFT != 0 {
        channels |= Channels::TOP_BACK_LEFT;
    }
    if channel_mask & SPEAKER_TOP_BACK_CENTER != 0 {
        channels |= Channels::TOP_BACK_CENTRE;
    }
    if channel_mask & SPEAKER_TOP_BACK_RIGHT != 0 {
        channels |= Channels::TOP_BACK_RIGHT;
    }

    channels
}
