#[cfg(feature = "peg")]
extern crate alloc;

#[cfg(feature = "ai-image-search")]
pub mod ai_image_search;
#[cfg(feature = "apk")]
pub mod apk;
#[cfg(feature = "crypto")]
pub mod crypto;
#[cfg(feature = "dlna")]
pub mod dlna;
#[cfg(feature = "file-walk")]
pub mod file_walk;
#[cfg(feature = "filesystem")]
pub mod filesystem;
#[cfg(feature = "media")]
pub mod media;
#[cfg(feature = "phone")]
pub mod phone;
pub mod platform;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "tls")]
pub mod tls;
pub mod utils;

#[cfg(any(test, feature = "test-seams"))]
#[path = "../../tests/plain-libs/support/tempdirs.rs"]
pub mod test_tempdirs;

pub mod protocols;

#[cfg(feature = "kv")]
pub mod kv;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(all(feature = "apk", any(test, feature = "test-seams")))]
#[path = "../../tests/plain-libs/support/apk.rs"]
pub mod test_apk;

pub mod case;
pub mod assertions;
pub mod streams;
#[cfg(feature = "peg")]
pub mod peg;

#[cfg(feature = "media")]
pub mod audio_tags;
#[cfg(feature = "thumbnails")]
pub mod image_resize;
#[cfg(feature = "ai-image-search")]
pub mod tokenizer;
#[cfg(feature = "native-watch")]
pub mod file_events;
#[cfg(any(feature = "disk-metrics", feature = "system-metrics"))]
pub mod system_metrics;
