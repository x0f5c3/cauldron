/// The MPEG audio version.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MPEGVersion {
    /// Version 2.5
    MPEG2p5,
    /// Version 2
    MPEG2,
    /// Version 1
    MPEG1,
}

/// The channel mode.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChannelMode {
    /// Single mono audio channel.
    Mono,
    /// Dual mono audio channels.
    DualMono,
    /// Stereo channels.
    Stereo,
    /// Joint Stereo encoded channels (decodes to Stereo).
    JointStereo { mid_side: bool, intensity: bool },
}

/// The emphasis applied during encoding.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Emphasis {
    /// No emphasis
    None,
    /// 50/15us
    Fifty15,
    /// CCIT J.17
    CcitJ17,
}

#[derive(Debug)]
pub struct FrameHeader {
    pub version: MPEGVersion,
    // number of bytes per second
    pub bitrate: u32,
    // number of decoded samples per second
    pub sample_rate: u32,
    pub channel_mode: ChannelMode,
    pub emphasis: Emphasis,
    pub has_padding: bool,
    // size of compressed frame data [in bytes]
    pub frame_size: usize,
    pub crc: Option<u16>,
}

impl FrameHeader {
    pub fn num_channels(&self) -> usize {
        if self.channel_mode == ChannelMode::Mono {
            1
        } else {
            2
        }
    }

    pub fn side_data_len(&self) -> usize {
        if self.channel_mode == ChannelMode::Mono && self.version != MPEGVersion::MPEG1 {
            9
        } else if self.channel_mode != ChannelMode::Mono && self.version == MPEGVersion::MPEG1 {
            32
        } else {
            17
        }
    }

    pub fn num_granules(&self) -> usize {
        if self.version == MPEGVersion::MPEG1 {
            2
        } else {
            1
        }
    }

    pub fn is_intensity_stereo(&self) -> bool {
        match self.channel_mode {
            ChannelMode::JointStereo {
                intensity: true, ..
            } => true,
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct FrameInfo {
    /// gives offset from which main data starts. For Layer III it can
    /// be negative implying that data of this frame can be found in
    /// previous frames. Headers, etc are not included in offset
    pub main_data_begin: u16,
    /// determines weather the same scale factors are
    /// transferred for both granules or not.
    pub scfsi: [[bool; 4]; 2],
    // Scale Factor Selection Information
    /// granules
    pub granules: [Granule; 2],
}

#[derive(Default)]
pub struct Granule {
    /// Each granule side info contains info about each channel
    pub channels: [GranuleChannel; 2],
}

pub struct GranuleChannel {
    /// number of bits for scalefactors[part2] and huffman data[part3]
    pub part2_3_length: u16,
    /// Each 576 samples are not encoded by same huffman table. It is divided
    /// into 5 regions i.e region0, region1, region2, count1, rzero.
    ///
    /// `big_values` contains first three regions and max value can
    /// be 576/2 = 288 samples.
    ///
    /// This field indicates max value of big_values partition.
    pub big_values: u16,
    /// Logarithmic quantization step size.
    pub global_gain: u8,
    /// number of bits used for the transmission of scalefactors
    pub scalefac_compress_len: u16,
    /// type of window used for the particular granule
    pub block_type: BlockType,
    /// Used when `BlockType` is Short
    ///
    /// Each 3 bit variable indicates the gain offset from global_gain for each
    /// short block
    pub subblock_gain: [u8; 3],
    /// The Huffman table to use for decoding region[0..3] of big_values.
    pub table_select: [u8; 3],
    /// The number of samples in region0 of big_values.
    pub region0_count: u8,
    /// The number of samples in region1 of big_values.
    pub region1_count: u8,
    /// Indicates if the pre-emphasis amount for each scale factor band should be
    /// added on to each scale factor before re-quantization.
    pub preflag: bool,
    /// A 0.5x (false) or 1x (true) multiplier for scale factors.
    pub scalefac_scale: bool,
    /// field determines out of two which huffman table to apply for count1 region
    pub count1table_select: bool,
    /// Long (scalefac_l) and short (scalefac_s) window scale factor bands.
    /// Must be interpreted based on the block type of the granule.
    ///
    /// For `block_type == BlockType::Short { is_mixed: false }`:
    ///   - scalefac_s[0..36] -> scalefacs[0..36]
    ///
    /// For `block_type == BlockType::Short { is_mixed: true }`:
    ///   - scalefac_l[0..8]  -> scalefacs[0..8]
    ///   - scalefac_s[0..27] -> scalefacs[8..35]
    ///
    /// For `block_type != BlockType::Short { .. }`:
    ///   - scalefac_l[0..21] -> scalefacs[0..21]
    ///
    /// Note: The standard doesn't explicitly call it out, but for Short blocks,
    ///       there are three additional scale factors, scalefacs[36..39], that
    ///       are always 0 and are not transmitted in the bitstream.
    ///
    /// For MPEG1, and MPEG2 without intensity stereo coding, a scale factor will
    /// not exceed 4 bits in length (maximum value 15).
    /// For MPEG2 with intensity stereo, a scale factor will not exceed 5 bits
    /// (maximum value 31) in length.
    pub scalefacs: [u8; 39],
    /// The starting sample index of the rzero partition, or the count of big_values
    /// and count1 samples.
    pub rzero: usize,
}

impl Default for GranuleChannel {
    fn default() -> Self {
        GranuleChannel {
            part2_3_length: 0,
            big_values: 0,
            global_gain: 0,
            scalefac_compress_len: 0,
            block_type: BlockType::Long,
            subblock_gain: [0; 3],
            table_select: [0; 3],
            region0_count: 0,
            region1_count: 0,
            preflag: false,
            scalefac_scale: false,
            count1table_select: false,
            scalefacs: [0; 39],
            rzero: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockType {
    Long,
    Start,
    /// if is mixed is true two lowest sub-bands are transformed using a normal window
    /// and the remaining 30 sub-bands are transformed using the window specified by the
    /// block_type
    Short {
        is_mixed: bool,
    },
    End,
}
