use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::error::{Id3v2Error, Id3v2ErrorKind, Result};
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::macros::try_vec;
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Read;

use crate::binary::{BigEndian, ReadBytesExt};

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("RVA2"));

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]

pub enum ChannelType {
    Other = 0,
    MasterVolume = 1,
    FrontRight = 2,
    FrontLeft = 3,
    BackRight = 4,
    BackLeft = 5,
    FrontCentre = 6,
    BackCentre = 7,
    Subwoofer = 8,
}

impl ChannelType {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Other),
            1 => Some(Self::MasterVolume),
            2 => Some(Self::FrontRight),
            3 => Some(Self::FrontLeft),
            4 => Some(Self::BackRight),
            5 => Some(Self::BackLeft),
            6 => Some(Self::FrontCentre),
            7 => Some(Self::BackCentre),
            8 => Some(Self::Subwoofer),
            _ => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ChannelInformation {
    pub channel_type: ChannelType,

    pub volume_adjustment: i16,

    pub bits_representing_peak: u8,

    pub peak_volume: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq)]
pub struct RelativeVolumeAdjustmentFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub identification: String,

    pub channels: HashMap<ChannelType, ChannelInformation>,
}

impl PartialEq for RelativeVolumeAdjustmentFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.identification == other.identification
    }
}

impl Hash for RelativeVolumeAdjustmentFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identification.hash(state)
    }
}

impl RelativeVolumeAdjustmentFrame<'_> {
    pub fn new(identification: String, channels: HashMap<ChannelType, ChannelInformation>) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            identification,
            channels,
        }
    }

    pub fn id(&self) -> FrameId<'_> {
        FRAME_ID
    }

    pub fn flags(&self) -> FrameFlags {
        self.header.flags
    }

    pub fn set_flags(&mut self, flags: FrameFlags) {
        self.header.flags = flags;
    }

    pub fn parse<R>(
        reader: &mut R,
        frame_flags: FrameFlags,
        parse_mode: ParsingMode,
    ) -> Result<Option<Self>>
    where
        R: Read,
    {
        let identification = decode_text(
            reader,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?
        .content;

        let mut channels = HashMap::new();
        while let Ok(channel_type_byte) = reader.read_u8() {
            let channel_type;
            match ChannelType::from_u8(channel_type_byte) {
                Some(channel_ty) => channel_type = channel_ty,
                None if parse_mode == ParsingMode::BestAttempt => channel_type = ChannelType::Other,
                _ => return Err(Id3v2Error::new(Id3v2ErrorKind::BadRva2ChannelType).into()),
            }

            let volume_adjustment = reader.read_i16::<BigEndian>()?;

            let bits_representing_peak = reader.read_u8()?;

            let mut peak_volume = None;
            if bits_representing_peak > 0 {
                let bytes_representing_peak = (u16::from(bits_representing_peak) + 7) >> 3;

                let mut peak_volume_bytes = try_vec![0; bytes_representing_peak as usize];
                reader.read_exact(&mut peak_volume_bytes)?;
                peak_volume = Some(peak_volume_bytes);
            }

            channels.insert(
                channel_type,
                ChannelInformation {
                    channel_type,
                    volume_adjustment,
                    bits_representing_peak,
                    peak_volume,
                },
            );
        }

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Some(Self {
            header,
            identification,
            channels,
        }))
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut content = Vec::new();

        content.extend(encode_text(
            &self.identification,
            TextEncoding::Latin1,
            true,
        ));

        for (channel_type, info) in &self.channels {
            let mut bits_representing_peak = info.bits_representing_peak;
            let expected_peak_byte_length = (u16::from(bits_representing_peak) + 7) >> 3;

            content.push(*channel_type as u8);
            content.extend(info.volume_adjustment.to_be_bytes());

            if info.peak_volume.is_none() {
                // Easiest path, no peak
                content.push(0);
                continue;
            }

            if let Some(peak) = &info.peak_volume {
                if peak.len() > expected_peak_byte_length as usize {
                    // Recalculate bits representing peak
                    bits_representing_peak = 0;

                    // Max out at 255 bits
                    for b in peak.iter().copied().take(32) {
                        bits_representing_peak += b.leading_ones() as u8;
                    }
                }

                content.push(bits_representing_peak);
                content.extend(peak.iter().take(32));
            }
        }

        content
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/relative_volume_adjustment_frame/tests.rs"]
mod tests;
