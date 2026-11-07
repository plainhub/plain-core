use super::read::PacketReader;
use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::error::Result;
use crate::audio_tags::macros::decode_err;
use crate::audio_tags::musepack::constants::FREQUENCY_TABLE;
use crate::audio_tags::properties::FileProperties;
use crate::audio_tags::util::math::RoundedDivision;

use std::io::Read;
use std::time::Duration;

use crate::utils::binary::{BigEndian, ReadBytesExt};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MpcSv8Properties {
    pub(crate) duration: Duration,
    pub(crate) average_bitrate: u32,

    pub stream_header: StreamHeader,

    pub replay_gain: ReplayGain,

    pub encoder_info: Option<EncoderInfo>,
}

impl From<MpcSv8Properties> for FileProperties {
    fn from(input: MpcSv8Properties) -> Self {
        Self {
            duration: input.duration,
            overall_bitrate: Some(input.average_bitrate),
            audio_bitrate: Some(input.average_bitrate),
            sample_rate: Some(input.stream_header.sample_rate),
            bit_depth: None,
            channels: Some(input.stream_header.channels),
            channel_mask: None,
        }
    }
}

impl MpcSv8Properties {
    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn average_bitrate(&self) -> u32 {
        self.average_bitrate
    }

    pub fn sample_rate(&self) -> u32 {
        self.stream_header.sample_rate
    }

    pub fn channels(&self) -> u8 {
        self.stream_header.channels
    }

    pub fn version(&self) -> u8 {
        self.stream_header.stream_version
    }

    pub(crate) fn read<R: Read>(reader: &mut R, parse_mode: ParsingMode) -> Result<Self> {
        super::read::read_from(reader, parse_mode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamHeader {
    pub crc: u32,

    pub stream_version: u8,

    pub sample_count: u64,

    pub beginning_silence: u64,

    pub sample_rate: u32,

    pub max_used_bands: u8,

    pub channels: u8,

    pub ms_used: bool,

    pub audio_block_frames: u16,
}

impl StreamHeader {
    pub(super) fn read<R: Read>(reader: &mut PacketReader<R>) -> Result<Self> {
        // StreamHeader format:
        //
        // Field              | Size (bits)     | Value | Comment
        // CRC                | 32              |       | CRC 32 of the block (this field excluded). 0 = invalid
        // Stream version     | 8               | 8     | Bitstream version
        // Sample count       | n*8; 0 < n < 10 |       | Number of samples in the stream. 0 = unknown
        // Beginning silence  | n*8; 0 < n < 10 |       | Number of samples to skip at the beginning of the stream
        // Sample frequency   | 3               | 0..7  | See table below
        // Max used bands     | 5               | 1..32 | Maximum number of bands used in the file
        // Channel count      | 4               | 1..16 | Number of channels in the stream
        // MS used            | 1               |       | True if Mid Side Stereo is enabled
        // Audio block frames | 3               | 0..7  | Number of frames per audio packet (4value=(1..16384))

        let crc = reader.read_u32::<BigEndian>()?;
        let stream_version = reader.read_u8()?;
        let (sample_count, _) = PacketReader::read_size(reader)?;
        let (beginning_silence, _) = PacketReader::read_size(reader)?;

        // Sample rate and max used bands
        let remaining_flags_byte_1 = reader.read_u8()?;

        let sample_rate_index = (remaining_flags_byte_1 & 0xE0) >> 5;
        let sample_rate = FREQUENCY_TABLE[sample_rate_index as usize];

        let max_used_bands = (remaining_flags_byte_1 & 0x1F) + 1;

        // Channel count, MS used, audio block frames
        let remaining_flags_byte_2 = reader.read_u8()?;

        let channels = (remaining_flags_byte_2 >> 4) + 1;
        let ms_used = remaining_flags_byte_2 & 0x08 == 0x08;

        let audio_block_frames_value = remaining_flags_byte_2 & 0x07;
        let audio_block_frames = 4u16.pow(u32::from(audio_block_frames_value));

        Ok(Self {
            crc,
            stream_version,
            sample_count,
            beginning_silence,
            sample_rate,
            max_used_bands,
            channels,
            ms_used,
            audio_block_frames,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]

pub struct ReplayGain {
    pub version: u8,

    pub title_gain: u16,
    pub title_peak: u16,

    pub album_gain: u16,
    pub album_peak: u16,
}

impl ReplayGain {
    pub(super) fn read<R: Read>(reader: &mut PacketReader<R>) -> Result<Self> {
        // ReplayGain format:
        //
        // Field 	          | Size (bits) | Value | Comment
        // ReplayGain version | 8           | 1     | The replay gain version
        // Title gain         | 16          |       | The loudness calculated for the title, and not the gain that the player must apply
        // Title peak         | 16          |       |
        // Album gain         | 16          |       | The loudness calculated for the album
        // Album peak         | 16          |       |

        let version = reader.read_u8()?;
        let title_gain = reader.read_u16::<BigEndian>()?;
        let title_peak = reader.read_u16::<BigEndian>()?;
        let album_gain = reader.read_u16::<BigEndian>()?;
        let album_peak = reader.read_u16::<BigEndian>()?;

        Ok(Self {
            version,
            title_gain,
            title_peak,
            album_gain,
            album_peak,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]

pub struct EncoderInfo {
    pub profile: f32,
    pub pns_tool: bool,

    pub major: u8,

    pub minor: u8,

    pub build: u8,
}

impl EncoderInfo {
    pub(super) fn read<R: Read>(reader: &mut PacketReader<R>) -> Result<Self> {
        // EncoderInfo format:
        //
        // Field 	| Size (bits) | Value
        // Profile 	| 7           | 0..15.875
        // PNS tool | 1           | True if enabled
        // Major 	| 8           | 1
        // Minor 	| 8           | 17
        // Build 	| 8           | 3

        let byte1 = reader.read_u8()?;
        let profile = f32::from((byte1 & 0xFE) >> 1) / 8.0;
        let pns_tool = byte1 & 0x01 == 1;

        let major = reader.read_u8()?;
        let minor = reader.read_u8()?;
        let build = reader.read_u8()?;

        Ok(Self {
            profile,
            pns_tool,
            major,
            minor,
            build,
        })
    }
}

pub(super) fn read(
    stream_length: u64,
    stream_header: StreamHeader,
    replay_gain: ReplayGain,
    encoder_info: Option<EncoderInfo>,
) -> Result<MpcSv8Properties> {
    let mut properties = MpcSv8Properties {
        duration: Duration::ZERO,
        average_bitrate: 0,
        stream_header,
        replay_gain,
        encoder_info,
    };

    let sample_count = stream_header.sample_count;
    let beginning_silence = stream_header.beginning_silence;
    let sample_rate = stream_header.sample_rate;

    if beginning_silence > sample_count {
        decode_err!(@BAIL Mpc, "Beginning silence is greater than the total sample count");
    }

    if sample_rate == 0 {
        log::warn!("Sample rate is 0, unable to calculate duration and bitrate");
        return Ok(properties);
    }

    if sample_count == 0 {
        log::warn!("Sample count is 0, unable to calculate duration and bitrate");
        return Ok(properties);
    }

    let total_samples = sample_count - beginning_silence;
    if total_samples == 0 {
        log::warn!(
            "Sample count (after removing beginning silence) is 0, unable to calculate duration \
			 and bitrate"
        );
        return Ok(properties);
    }

    let length = (total_samples * 1000).div_round(u64::from(sample_rate));

    properties.duration = Duration::from_millis(length);
    properties.average_bitrate =
        ((stream_length * 8 * u64::from(sample_rate)) / (total_samples * 1000)) as u32;

    Ok(properties)
}
