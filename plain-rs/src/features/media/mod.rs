//! Shared user-library domain — SQLite-backed audio playback queue /
//! playlists / play history, tags + tag relations and favorite folders.
//!

pub mod duration_cache;

pub mod actions;

pub mod images;

#[cfg(feature = "library")]
pub mod play_history;

pub mod audio;

pub mod catalog_paths;
pub mod file_metadata;

#[cfg(feature = "image-metadata")]
pub mod file_facts;
