use crate::audio_tags::error::Result;
use crate::audio_tags::macros::decode_err;
use crate::audio_tags::musepack::constants::{
    FREQUENCY_TABLE, MPC_DECODER_SYNTH_DELAY, MPC_FRAME_LENGTH, MPC_OLD_GAIN_REF,
};
use crate::audio_tags::properties::FileProperties;

use std::io::Read;
use std::time::Duration;

use crate::binary::{LittleEndian, ReadBytesExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    None,

    Unstable,

    Unused,

    BelowTelephone0,

    BelowTelephone1,

    Telephone,

    Thumb,

    Radio,

    Standard,

    Xtreme,

    Insane,

    BrainDead,

    AboveBrainDead9,

    AboveBrainDead10,
}

impl Profile {
    #[rustfmt::skip]
    pub fn from_u8(value: u8) -> Option<Self> {
		match value {
			0         => Some(Self::None),
			1         => Some(Self::Unstable),
			2 | 3 | 4 => Some(Self::Unused),
			5         => Some(Self::BelowTelephone0),
			6         => Some(Self::BelowTelephone1),
			7         => Some(Self::Telephone),
			8         => Some(Self::Thumb),
			9         => Some(Self::Radio),
			10        => Some(Self::Standard),
			11        => Some(Self::Xtreme),
			12        => Some(Self::Insane),
			13        => Some(Self::BrainDead),
			14        => Some(Self::AboveBrainDead9),
			15        => Some(Self::AboveBrainDead10),
			_         => None,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Link {
    #[default]
    VeryLowStartOrEnd,

    LoudEnd,

    LoudStart,

    LoudStartAndEnd,
}

impl Link {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::VeryLowStartOrEnd),
            1 => Some(Self::LoudEnd),
            2 => Some(Self::LoudStart),
            3 => Some(Self::LoudStartAndEnd),
            _ => None,
        }
    }
}

// http://trac.musepack.net/musepack/wiki/SV7Specification

#[derive(Debug, Clone, PartialEq, Default)]

pub struct MpcSv7Properties {
    pub(crate) duration: Duration,
    pub(crate) average_bitrate: u32,
    pub(crate) channels: u8, // NOTE: always 2
    // -- Section 1 --
    pub(crate) frame_count: u32,
    // -- Section 2 --
    pub(crate) intensity_stereo: bool,
    pub(crate) mid_side_stereo: bool,
    pub(crate) max_band: u8,
    pub(crate) profile: Profile,
    pub(crate) link: Link,
    pub(crate) sample_freq: u32,
    pub(crate) max_level: u16,
    // -- Section 3 --
    pub(crate) title_gain: i16,
    pub(crate) title_peak: u16,
    // -- Section 4 --
    pub(crate) album_gain: i16,
    pub(crate) album_peak: u16,
    // -- Section 5 --
    pub(crate) true_gapless: bool,
    pub(crate) last_frame_length: u16,
    pub(crate) fast_seeking_safe: bool,
    // -- Section 6 --
    pub(crate) encoder_version: u8,
}

impl From<MpcSv7Properties> for FileProperties {
    fn from(input: MpcSv7Properties) -> Self {
        Self {
            duration: input.duration,
            overall_bitrate: Some(input.average_bitrate),
            audio_bitrate: Some(input.average_bitrate),
            sample_rate: Some(input.sample_freq),
            bit_depth: None,
            channels: Some(input.channels),
            channel_mask: None,
        }
    }
}

impl MpcSv7Properties {
    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn average_bitrate(&self) -> u32 {
        self.average_bitrate
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_freq
    }

    pub fn channels(&self) -> u8 {
        self.channels
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    pub fn intensity_stereo(&self) -> bool {
        self.intensity_stereo
    }

    pub fn mid_side_stereo(&self) -> bool {
        self.mid_side_stereo
    }

    pub fn max_band(&self) -> u8 {
        self.max_band
    }

    pub fn profile(&self) -> Profile {
        self.profile
    }

    pub fn link(&self) -> Link {
        self.link
    }

    pub fn max_level(&self) -> u16 {
        self.max_level
    }

    pub fn title_gain(&self) -> i16 {
        self.title_gain
    }

    pub fn title_peak(&self) -> u16 {
        self.title_peak
    }

    pub fn album_gain(&self) -> i16 {
        self.album_gain
    }

    pub fn album_peak(&self) -> u16 {
        self.album_peak
    }

    pub fn true_gapless(&self) -> bool {
        self.true_gapless
    }

    pub fn last_frame_length(&self) -> u16 {
        self.last_frame_length
    }

    pub fn fast_seeking_safe(&self) -> bool {
        self.fast_seeking_safe
    }

    pub fn encoder_version(&self) -> u8 {
        self.encoder_version
    }

    pub(crate) fn read<R>(reader: &mut R, stream_length: u64) -> Result<Self>
    where
        R: Read,
    {
        let version = reader.read_u8()?;
        if version & 0x0F != 7 {
            decode_err!(@BAIL Mpc, "Expected stream version 7");
        }

        let mut properties = MpcSv7Properties {
            channels: 2, // Always 2 channels
            ..Self::default()
        };

        // TODO: Make a Bitreader, would be nice crate-wide but especially here
        // The SV7 header is split into 6 32-bit sections

        // -- Section 1 --
        properties.frame_count = reader.read_u32::<LittleEndian>()?;

        // -- Section 2 --
        let chunk = reader.read_u32::<LittleEndian>()?;

        let byte1 = ((chunk & 0xFF00_0000) >> 24) as u8;

        properties.intensity_stereo = ((byte1 & 0x80) >> 7) == 1;
        properties.mid_side_stereo = ((byte1 & 0x40) >> 6) == 1;
        properties.max_band = byte1 & 0x3F;

        let byte2 = ((chunk & 0xFF_0000) >> 16) as u8;

        properties.profile = Profile::from_u8((byte2 & 0xF0) >> 4).unwrap(); // Infallible
        properties.link = Link::from_u8((byte2 & 0x0C) >> 2).unwrap(); // Infallible

        let sample_freq_index = byte2 & 0x03;
        properties.sample_freq = FREQUENCY_TABLE[sample_freq_index as usize];

        let remaining_bytes = (chunk & 0xFFFF) as u16;
        properties.max_level = remaining_bytes;

        // -- Section 3 --
        let title_peak = reader.read_u16::<LittleEndian>()?;
        let title_gain = reader.read_u16::<LittleEndian>()?;

        // -- Section 4 --
        let album_peak = reader.read_u16::<LittleEndian>()?;
        let album_gain = reader.read_u16::<LittleEndian>()?;

        // -- Section 5 --
        let chunk = reader.read_u32::<LittleEndian>()?;

        properties.true_gapless = (chunk >> 31) == 1;

        if properties.true_gapless {
            properties.last_frame_length = ((chunk >> 20) & 0x7FF) as u16;
        }

        properties.fast_seeking_safe = (chunk >> 19) & 1 == 1;

        // NOTE: Rest of the chunk is zeroed and unused

        // -- Section 6 --
        properties.encoder_version = reader.read_u8()?;

        // -- End of parsing --

        // Convert ReplayGain values
        let set_replay_gain = |gain: u16| -> i16 {
            if gain == 0 {
                return 0;
            }

            let gain = ((MPC_OLD_GAIN_REF - f32::from(gain) / 100.0) * 256.0 + 0.5) as i16;
            if !(0..i16::MAX).contains(&gain) {
                return 0;
            }
            gain
        };
        let set_replay_peak = |peak: u16| -> u16 {
            if peak == 0 {
                return 0;
            }

            ((f64::from(peak).log10() * 20.0 * 256.0) + 0.5) as u16
        };

        properties.title_gain = set_replay_gain(title_gain);
        properties.title_peak = set_replay_peak(title_peak);
        properties.album_gain = set_replay_gain(album_gain);
        properties.album_peak = set_replay_peak(album_peak);

        if properties.last_frame_length > MPC_FRAME_LENGTH as u16 {
            decode_err!(@BAIL Mpc, "Invalid last frame length");
        }

        if properties.sample_freq == 0 {
            log::warn!("Sample rate is 0, unable to calculate duration and bitrate");
            return Ok(properties);
        }

        if properties.frame_count == 0 {
            log::warn!("Frame count is 0, unable to calculate duration and bitrate");
            return Ok(properties);
        }

        let time_per_frame = (MPC_FRAME_LENGTH as f64) / f64::from(properties.sample_freq);
        let length = (f64::from(properties.frame_count) * time_per_frame) * 1000.0;
        properties.duration = Duration::from_millis(length as u64);

        let total_samples;
        if properties.true_gapless {
            total_samples = (u64::from(properties.frame_count) * MPC_FRAME_LENGTH)
                - (MPC_FRAME_LENGTH - u64::from(properties.last_frame_length));
        } else {
            total_samples =
                (u64::from(properties.frame_count) * MPC_FRAME_LENGTH) - MPC_DECODER_SYNTH_DELAY;
        }

        properties.average_bitrate = ((stream_length * 8 * u64::from(properties.sample_freq))
            / (total_samples * 1000)) as u32;

        Ok(properties)
    }
}
