#[cfg(feature = "auth")]
pub mod auth;

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
#[cfg(any(feature = "ai-image-search", feature = "document-nfc"))]
pub mod normalization;
#[cfg(feature = "phone")]
pub mod phone;
#[cfg(not(target_arch = "wasm32"))]
pub mod platform;
#[cfg(feature = "time")]
pub mod local_time;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "tls")]
pub mod tls;
#[cfg(not(target_arch = "wasm32"))]
pub mod utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod temporary;

#[cfg(any(test, feature = "test-seams"))]
#[path = "../../tests/plain-libs/support/tempdirs.rs"]
pub mod test_tempdirs;

#[cfg(not(target_arch = "wasm32"))]
pub mod protocols;

#[cfg(feature = "kv")]
pub mod kv;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(all(feature = "apk", any(test, feature = "test-seams")))]
#[path = "../../tests/plain-libs/support/apk.rs"]
pub mod test_apk;

#[cfg(feature = "compound-file")]
pub mod compound_file;
#[cfg(not(target_arch = "wasm32"))]
pub mod case;
#[cfg(not(target_arch = "wasm32"))]
pub mod assertions;
#[cfg(not(target_arch = "wasm32"))]
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
