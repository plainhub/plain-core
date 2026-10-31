//! Shared user-library domain — SQLite-backed audio playback queue /
//! playlists / play history, tags + tag relations and favorite folders.
//!

pub mod duration_cache;
pub mod isobmff;

pub mod actions;

pub mod images;

#[cfg(feature = "library")]
pub mod play_history;

pub mod audio;

pub mod documents;

pub mod catalog_paths;
pub mod file_metadata;
pub mod probe;

pub mod cover;

mod raw_audio;

mod asf;

mod midi;

pub mod video_info;

mod matroska;

mod avi;

mod flv;

mod h264;

mod bits;

mod transport_stream;

mod hevc;

#[cfg(feature = "image-metadata")]
pub mod file_facts;

#[cfg(feature = "media")]
pub mod thumbnails;
