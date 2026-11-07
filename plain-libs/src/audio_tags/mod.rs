#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

// proc macro hacks

pub mod config;
pub mod error;
pub mod file;
pub(crate) mod macros;
pub mod picture;
pub mod probe;
pub mod properties;
pub mod resolve;
pub mod tag;
mod util;

pub mod aac;
pub mod ape;
pub mod flac;
pub mod id3;
pub mod iff;
pub mod mp4;
pub mod mpeg;
pub mod musepack;
pub mod ogg;
pub mod wavpack;

pub use crate::audio_tags::probe::{read_from, read_from_path};

pub use util::text::TextEncoding;


pub use util::io;

pub mod prelude {

    pub use crate::audio_tags::file::{AudioFile, TaggedFileExt};
    pub use crate::audio_tags::tag::{Accessor, ItemKey, MergeTag, SplitTag, TagExt};
}
