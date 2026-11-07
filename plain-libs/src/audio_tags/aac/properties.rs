use crate::audio_tags::aac::header::ADTSHeader;
use crate::audio_tags::mp4::AudioObjectType;
use crate::audio_tags::mpeg::header::MpegVersion;
use crate::audio_tags::properties::{ChannelMask, FileProperties};

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AACProperties {
    pub(crate) version: MpegVersion,
    pub(crate) audio_object_type: AudioObjectType,
    pub(crate) duration: Duration,
    pub(crate) overall_bitrate: u32,
    pub(crate) audio_bitrate: u32,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u8,
    pub(crate) channel_mask: Option<ChannelMask>,
    pub(crate) copyright: bool,
    pub(crate) original: bool,
}

impl AACProperties {
    pub fn version(&self) -> MpegVersion {
        self.version
    }

    pub fn audio_object_type(&self) -> AudioObjectType {
        self.audio_object_type
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn overall_bitrate(&self) -> u32 {
        self.overall_bitrate
    }

    pub fn audio_bitrate(&self) -> u32 {
        self.audio_bitrate
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u8 {
        self.channels
    }

    pub fn channel_mask(&self) -> Option<ChannelMask> {
        self.channel_mask
    }

    pub fn copyright(&self) -> bool {
        self.copyright
    }

    pub fn original(&self) -> bool {
        self.original
    }
}

impl From<AACProperties> for FileProperties {
    fn from(input: AACProperties) -> Self {
        FileProperties {
            duration: input.duration,
            overall_bitrate: Some(input.overall_bitrate),
            audio_bitrate: Some(input.audio_bitrate),
            sample_rate: Some(input.sample_rate),
            bit_depth: None,
            channels: Some(input.channels),
            channel_mask: input.channel_mask,
        }
    }
}

pub(super) fn read_properties(
    properties: &mut AACProperties,
    first_frame: ADTSHeader,
    stream_len: u64,
) {
    properties.version = first_frame.version;
    properties.audio_object_type = first_frame.audio_object_ty;
    properties.sample_rate = first_frame.sample_rate;
    properties.channels = first_frame.channels;

    match ChannelMask::from_mp4_channels(properties.channels) {
        Some(mask) => properties.channel_mask = Some(mask),
        None => {
            log::warn!(
                "Unable to create channel mask, invalid channel count: {}",
                properties.channels
            );
        }
    }

    properties.copyright = first_frame.copyright;
    properties.original = first_frame.original;

    let bitrate = first_frame.bitrate;

    if bitrate > 0 {
        properties.audio_bitrate = bitrate;
        properties.overall_bitrate = bitrate;
        properties.duration = Duration::from_millis((stream_len * 8) / u64::from(bitrate));
    }
}
