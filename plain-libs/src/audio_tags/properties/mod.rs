mod channel_mask;
mod file_properties;

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/properties/mod/tests.rs"]
mod tests;

pub use channel_mask::ChannelMask;
pub use file_properties::FileProperties;
