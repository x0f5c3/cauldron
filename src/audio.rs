//! `audio` is the main module for audio decoders.

use bitflags::bitflags;
use std::fmt;

use super::io::{
    AudioInputStream, AudioReader, AudioSamplesIterator, IntoAudioInputStream, Sample,
};
use super::{codecs, errors, Result};
use super::{flac, mp3, wav};

bitflags! {
    /// Channels is a bit mask of all channels contained in a signal.
    /// see https://trac.ffmpeg.org/wiki/AudioChannelManipulation for more info
    pub struct Channels: u32 {
        const FRONT_LEFT         = 0x0000_0001; // Mono Channel
        const FRONT_RIGHT        = 0x0000_0002; // Stereo channel
        const FRONT_CENTRE       = 0x0000_0004;
        const BACK_LEFT          = 0x0000_0008;
        const BACK_CENTRE        = 0x0000_0010;
        const BACK_RIGHT         = 0x0000_0020;
        const LFE1               = 0x0000_0040; // Low frequency channel 1.
        const FRONT_LEFT_CENTRE  = 0x0000_0080;
        const FRONT_RIGHT_CENTRE = 0x0000_0100;
        const BACK_LEFT_CENTRE   = 0x0000_0200;
        const BACK_RIGHT_CENTRE  = 0x0000_0400;
        const FRONT_LEFT_WIDE    = 0x0000_0800;
        const FRONT_RIGHT_WIDE   = 0x0000_1000;
        const FRONT_LEFT_HIGH    = 0x0000_2000;
        const FRONT_CENTRE_HIGH  = 0x0000_4000;
        const FRONT_RIGHT_HIGH   = 0x0000_8000;
        const LFE2               = 0x0001_0000;  // Low frequency channel 2
        const SIDE_LEFT          = 0x0002_0000;
        const SIDE_RIGHT         = 0x0004_0000;
        const TOP_CENTRE         = 0x0008_0000;
        const TOP_FRONT_LEFT     = 0x0010_0000;
        const TOP_FRONT_CENTRE   = 0x0020_0000;
        const TOP_FRONT_RIGHT    = 0x0040_0000;
        const TOP_BACK_LEFT      = 0x0080_0000;
        const TOP_BACK_CENTRE    = 0x0100_0000;
        const TOP_BACK_RIGHT     = 0x0200_0000;
    }
}

impl Channels {
    /// Gets the number of channels.
    pub fn count(self) -> usize {
        self.bits.count_ones() as usize
    }
}

impl fmt::Display for Channels {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#032b}", self.bits)
    }
}

/// `ChannelLayout` describes common audio channel configurations.
/// Run `ffmpeg -layouts` to see the layout mappings
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChannelLayout {
    /// single channel stream
    Mono,
    /// dual channel stream, corresponds with Left and Right
    Stereo,
    /// apart from dual channel, also contains LFE channel for bass effects
    TwoPointOne,
    /// front channel sound - FL + FR + FC
    ThreePointZero,
    /// four channel sound, uses four surrounding directions for speakers
    Quad,
    /// full surround sound
    FivePointZero,
    /// 5.1 surround sound with one LFE Channel
    FivePointOne,
    /// surround sound with more focus on front speakers
    SixPointOne,
    /// surround sound with more focus on back speakers
    SixPointOneBack,
    /// 7.1 surround sound - for theaters and home cinema
    SevenPointOne,
}

impl ChannelLayout {
    /// Converts a channel `ChannelLayout` into a `Channels` bit mask.
    pub fn into_channels(self) -> Channels {
        match self {
            ChannelLayout::Mono => Channels::FRONT_LEFT,
            ChannelLayout::Stereo => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
            ChannelLayout::TwoPointOne => {
                Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::LFE1
            }
            ChannelLayout::ThreePointZero => {
                Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::FRONT_CENTRE
            }
            ChannelLayout::Quad => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
            }
            ChannelLayout::FivePointZero => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::FRONT_CENTRE
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
            }
            ChannelLayout::FivePointOne => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::FRONT_CENTRE
                    | Channels::LFE1
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
            }
            ChannelLayout::SixPointOne => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::FRONT_CENTRE
                    | Channels::LFE1
                    | Channels::BACK_CENTRE
                    | Channels::SIDE_LEFT
                    | Channels::SIDE_RIGHT
            }
            ChannelLayout::SixPointOneBack => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::FRONT_CENTRE
                    | Channels::LFE1
                    | Channels::BACK_CENTRE
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
            }
            ChannelLayout::SevenPointOne => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::FRONT_CENTRE
                    | Channels::LFE1
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
                    | Channels::SIDE_LEFT
                    | Channels::SIDE_RIGHT
            }
        }
    }
}

impl fmt::Display for ChannelLayout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// AudioInfo stored in a container format's headers and metadata
#[derive(Debug)]
pub struct AudioInfo {
    /// Codec of the audio
    pub codec_type: codecs::CodecType,

    /// The sample rate of the audio in Hz.
    pub sample_rate: u32,

    /// The length of the encoded stream in number of frames.
    pub total_samples: u64,

    /// The number of bits per one decoded audio sample.
    pub bits_per_sample: u32,

    /// A list of in-order channels.
    pub channels: Channels,

    /// The channel layout.
    pub channel_layout: ChannelLayout,
}

impl fmt::Display for AudioInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "| CodecType:             {}", self.codec_type)?;
        writeln!(f, "| Sample Rate:           {}", self.sample_rate)?;
        writeln!(f, "| Bits per Sample:       {}", self.bits_per_sample)?;
        writeln!(f, "| Channel(s):            {}", self.channels.count())?;
        writeln!(f, "| Channel Layout:        {:?}", self.channel_layout)?;

        Ok(())
    }
}

/// Type for sample iterator returned by `AudioSegment`
pub type SampleIterator<'a, S> = Box<dyn AudioSamplesIterator<S> + 'a>;

/// `AudioSegment` is returned to user to perform various operations and get
/// decoded stream, audio info or encode to different format.
pub struct AudioSegment {
    /// codec flag
    codec_flag: codecs::FormatFlag,

    /// audio info stored in a container format's headers and metadata
    info: AudioInfo,

    /// audio reader
    reader: Box<dyn AudioReader>,

    /// flag is set when samples iterator is returned
    is_buffer_used: bool,
}

impl AudioSegment {
    //noinspection TodoComment
    //noinspection TodoComment
    /// Constructs a new `AudioSegment`.
    ///
    /// # Example
    ///
    /// ```
    /// use cauldron::audio::AudioSegment;
    /// use cauldron::codecs::FormatFlag;
    ///
    /// match AudioSegment::read("tests/samples/wav/test-s16le-44100Hz-mono.wav") {
    ///   Ok(f)  => f,
    ///   Err(e) => panic!("Couldn't open example file: {}", e)
    /// };
    /// ```

    /// read audio file from file path and returns `AudioSegment`
    ///
    /// Determines the format from the file extension
    ///
    /// TODO: use audio metadata to determine the format
    pub fn read(filename: &str) -> Result<AudioSegment> {
        let flag = AudioSegment::get_format_flag(filename)?;

        AudioSegment::read_with_format(filename, flag)
    }

    /// Read audio file from file path and returns `AudioSegment`
    ///
    /// You can pass file path as `String, &str or &std::path::Path`
    ///
    /// ```
    /// use cauldron::audio::AudioSegment;
    /// use cauldron::codecs::FormatFlag;
    ///
    /// match AudioSegment::read_with_format(
    ///     std::path::Path::new("tests/samples/wav/test-s16le-44100Hz-mono.wav"), FormatFlag::WAV) {
    ///   Ok(f)  => f,
    ///   Err(e) => panic!("Couldn't open example file: {}", e)
    /// };
    /// ```
    ///
    /// Irrespective of file extension, it uses the provided format flag
    pub fn read_with_format<I: IntoAudioInputStream>(
        data: I,
        flag: codecs::FormatFlag,
    ) -> Result<AudioSegment> {
        return AudioSegment::create_audio_segment(data.into_stream()?, flag);
    }

    fn create_audio_segment(
        input: AudioInputStream,
        format_flag: codecs::FormatFlag,
    ) -> Result<AudioSegment> {
        let mut read_res: Box<dyn AudioReader> = match format_flag {
            codecs::FormatFlag::WAV => wav::WavReader::new(input)?,
            codecs::FormatFlag::FLAC => flac::FlacReader::new(input)?,
            codecs::FormatFlag::MP3 => mp3::Mp3Reader::new(input)?,
            _ => return errors::unsupported_error("Codec flag not supported"),
        };

        Ok(AudioSegment {
            codec_flag: format_flag,
            info: read_res.read_header()?,
            reader: read_res,
            is_buffer_used: false,
        })
    }

    /// returns audio info as `AudioInfo`
    pub fn info(&self) -> &AudioInfo {
        &self.info
    }

    /// returns number of channels in the audio
    pub fn number_channels(&self) -> usize {
        self.info.channels.count()
    }

    /// Returns the duration of the audio file in seconds
    ///
    /// duration = (total_samples / no_channels) / sampling_rate
    pub fn duration(&self) -> f32 {
        self.info.total_samples as f32
            / (self.number_channels() as u32 * self.info.sample_rate) as f32
    }

    /// Returns bitrate of the audio in kbps
    pub fn bitrate(&self) -> u32 {
        (self.info.sample_rate / 1000) * self.info.bits_per_sample * self.number_channels() as u32
    }

    /// Returns an channel interleaved iterator on samples
    pub fn samples<'a, S: Sample + 'a>(&'a mut self) -> Result<SampleIterator<'a, S>> {
        if self.is_buffer_used {
            return errors::unsupported_error("requesting iterator again");
        }
        self.is_buffer_used = true;
        let itr = match self.codec_flag {
            codecs::FormatFlag::WAV => wav::WavSamplesIterator::new(&mut self.reader, &self.info),
            codecs::FormatFlag::FLAC => {
                flac::FlacSamplesIterator::new(&mut self.reader, &self.info)
            }
            codecs::FormatFlag::MP3 => mp3::Mp3SamplesIterator::new(&mut self.reader, &self.info),
            _ => unreachable!(),
        };
        Ok(itr)
    }

    fn get_format_flag(filename: &str) -> Result<codecs::FormatFlag> {
        let extension = match filename.split('.').last() {
            Some(ex) => ex,
            None => return errors::unsupported_error("no decoder flag found for given file"),
        };
        match extension {
            "wav" => Ok(codecs::FormatFlag::WAV),
            "flac" => Ok(codecs::FormatFlag::FLAC),
            "mp3" => Ok(codecs::FormatFlag::MP3),
            "aac" => Ok(codecs::FormatFlag::AAC),
            "ogg" => Ok(codecs::FormatFlag::VORBIS),
            "raw" => Ok(codecs::FormatFlag::PCM),
            "pcm" => Ok(codecs::FormatFlag::PCM),
            _ => errors::unsupported_error("no decoder flag found for given file"),
        }
    }
}

impl fmt::Display for AudioSegment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AudioInfo:\n{}\n", self.info)?;
        write!(
            f,
            "duration: {}s, bitrate: {} kb/s",
            self.duration(),
            self.bitrate()
        )?;
        Ok(())
    }
}
