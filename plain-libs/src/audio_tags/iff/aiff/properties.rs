use super::read::CompressionPresent;
use crate::audio_tags::error::Result;
use crate::audio_tags::macros::{decode_err, try_vec};
use crate::audio_tags::properties::FileProperties;
use crate::audio_tags::util::text::utf8_decode;

use std::borrow::Cow;
use std::io::Read;
use std::time::Duration;

use crate::audio_tags::io::ReadExt;
use crate::utils::binary::{BigEndian, ReadBytesExt};

#[derive(Clone, Eq, PartialEq, Default, Debug)]
pub enum AiffCompressionType {
    #[default]
    None,

    ACE2,

    ACE8,

    MAC3,

    MAC6,

    sowt,

    fl32,

    fl64,

    alaw,

    ulaw,

    ULAW,

    ALAW,

    FL32,

    Other {
        compression_type: [u8; 4],

        compression_name: String,
    },
}

impl AiffCompressionType {
    pub fn compression_name(&self) -> Cow<'_, str> {
        match self {
            AiffCompressionType::None => Cow::Borrowed("not compressed"),
            AiffCompressionType::ACE2 => Cow::Borrowed("ACE 2-to-1"),
            AiffCompressionType::ACE8 => Cow::Borrowed("ACE 8-to-3"),
            AiffCompressionType::MAC3 => Cow::Borrowed("MACE 3-to-1"),
            AiffCompressionType::MAC6 => Cow::Borrowed("MACE 6-to-1"),
            AiffCompressionType::sowt => Cow::Borrowed(""), // Has no compression name
            AiffCompressionType::fl32 => Cow::Borrowed("32-bit floating point"),
            AiffCompressionType::fl64 => Cow::Borrowed("64-bit floating point"),
            AiffCompressionType::alaw => Cow::Borrowed("ALaw 2:1"),
            AiffCompressionType::ulaw => Cow::Borrowed("µLaw 2:1"),
            AiffCompressionType::ULAW => Cow::Borrowed("CCITT G.711 u-law"),
            AiffCompressionType::ALAW => Cow::Borrowed("CCITT G.711 A-law"),
            AiffCompressionType::FL32 => Cow::Borrowed("Float 32"),
            AiffCompressionType::Other {
                compression_name, ..
            } => Cow::from(compression_name),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
#[non_exhaustive]
pub struct AiffProperties {
    pub(crate) duration: Duration,
    pub(crate) overall_bitrate: u32,
    pub(crate) audio_bitrate: u32,
    pub(crate) sample_rate: u32,
    pub(crate) sample_size: u16,
    pub(crate) channels: u16,
    pub(crate) compression_type: Option<AiffCompressionType>,
}

impl From<AiffProperties> for FileProperties {
    fn from(value: AiffProperties) -> Self {
        Self {
            duration: value.duration,
            overall_bitrate: Some(value.overall_bitrate),
            audio_bitrate: Some(value.audio_bitrate),
            sample_rate: Some(value.sample_rate),
            bit_depth: Some(value.sample_size as u8),
            channels: Some(value.channels as u8),
            channel_mask: None,
        }
    }
}

impl AiffProperties {
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

    pub fn sample_size(&self) -> u16 {
        self.sample_size
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn compression_type(&self) -> Option<&AiffCompressionType> {
        self.compression_type.as_ref()
    }
}

pub(super) fn read_properties(
    comm: &mut &[u8],
    compression_present: CompressionPresent,
    stream_len: u32,
    file_length: u64,
) -> Result<AiffProperties> {
    let channels = comm.read_u16::<BigEndian>()?;

    if channels == 0 {
        decode_err!(@BAIL Aiff, "File contains 0 channels");
    }

    let sample_frames = comm.read_u32::<BigEndian>()?;
    let sample_size = comm.read_u16::<BigEndian>()?;

    let sample_rate_extended = comm.read_f80()?;
    let sample_rate_64 = sample_rate_extended.as_f64();
    if !sample_rate_64.is_finite() || !sample_rate_64.is_sign_positive() {
        decode_err!(@BAIL Aiff, "Invalid sample rate");
    }

    let sample_rate = sample_rate_64.round() as u32;

    let (duration, overall_bitrate, audio_bitrate) = if sample_rate > 0 && sample_frames > 0 {
        let length = (f64::from(sample_frames) * 1000.0) / f64::from(sample_rate);

        (
            Duration::from_millis(length as u64),
            ((file_length as f64) * 8.0 / length + 0.5) as u32,
            (f64::from(stream_len) * 8.0 / length + 0.5) as u32,
        )
    } else {
        (Duration::ZERO, 0, 0)
    };

    let is_compressed = comm.len() >= 5 && compression_present == CompressionPresent::Yes;
    if !is_compressed {
        return Ok(AiffProperties {
            duration,
            overall_bitrate,
            audio_bitrate,
            sample_rate,
            sample_size,
            channels,
            compression_type: None,
        });
    }

    let mut compression_type = [0u8; 4];
    comm.read_exact(&mut compression_type)?;

    let compression = Some(match &compression_type {
        b"NONE" => AiffCompressionType::None,
        b"ACE2" => AiffCompressionType::ACE2,
        b"ACE8" => AiffCompressionType::ACE8,
        b"MAC3" => AiffCompressionType::MAC3,
        b"MAC6" => AiffCompressionType::MAC6,
        b"sowt" => AiffCompressionType::sowt,
        b"fl32" => AiffCompressionType::fl32,
        b"fl64" => AiffCompressionType::fl64,
        b"alaw" => AiffCompressionType::alaw,
        b"ulaw" => AiffCompressionType::ulaw,
        b"ULAW" => AiffCompressionType::ULAW,
        b"ALAW" => AiffCompressionType::ALAW,
        b"FL32" => AiffCompressionType::FL32,
        _ => {
            log::debug!(
                "Encountered unknown compression type: {:?}",
                compression_type
            );

            // We have to read the compression name string
            let mut compression_name = String::new();

            let compression_name_size = comm.read_u8()?;
            if compression_name_size > 0 {
                let mut compression_name_bytes = try_vec![0u8; compression_name_size as usize];
                comm.read_exact(&mut compression_name_bytes)?;

                compression_name = utf8_decode(compression_name_bytes)?;
            }

            AiffCompressionType::Other {
                compression_type,
                compression_name,
            }
        }
    });

    Ok(AiffProperties {
        duration,
        overall_bitrate,
        audio_bitrate,
        sample_rate,
        sample_size,
        channels,
        compression_type: compression,
    })
}
