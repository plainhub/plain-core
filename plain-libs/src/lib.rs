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
#[cfg(feature = "time")]
pub mod local_time;
#[cfg(feature = "media")]
pub mod media;
#[cfg(any(feature = "ai-image-search", feature = "document-nfc"))]
pub mod normalization;
#[cfg(feature = "phone")]
pub mod phone;
#[cfg(not(target_arch = "wasm32"))]
pub mod platform;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(not(target_arch = "wasm32"))]
pub mod temporary;
#[cfg(feature = "tls")]
pub mod tls;
#[cfg(not(target_arch = "wasm32"))]
pub mod utils;

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

#[cfg(not(target_arch = "wasm32"))]
pub mod assertions;
#[cfg(not(target_arch = "wasm32"))]
pub mod case;
#[cfg(feature = "compound-file")]
pub mod compound_file;
#[cfg(feature = "peg")]
pub mod peg;
#[cfg(not(target_arch = "wasm32"))]
pub mod streams;

#[cfg(feature = "media")]
pub mod audio_tags;
#[cfg(feature = "native-watch")]
pub mod file_events;
#[cfg(feature = "thumbnails")]
pub mod image_resize;
#[cfg(any(feature = "disk-metrics", feature = "system-metrics"))]
pub mod system_metrics;
#[cfg(feature = "ai-image-search")]
pub mod tokenizer;

#[cfg(feature = "thumbnails")]
mod jpeg;

#[cfg(feature = "xml")]
pub mod xml;

#[cfg(feature = "thumbnails")]
mod h264;

pub mod anyhow;
#[cfg(feature = "image")]
mod byte_order;
#[cfg(feature = "time")]
pub mod chrono;
#[cfg(feature = "image")]
pub mod image;
#[cfg(feature = "image")]
mod image_codecs;
#[cfg(feature = "regex")]
mod regex_engine;
#[cfg(feature = "zip")]
pub mod zip;

#[macro_export]
macro_rules! plain_error_ensure {
    ($condition:expr, $($message:tt)+) => {
        if !$condition {
            return $crate::anyhow::__private::Err($crate::anyhow::__anyhow!($($message)+));
        }
    };
}

#[cfg(feature = "image")]
mod byte_cast;
#[cfg(feature = "compression")]
mod checksum;
#[cfg(feature = "compression")]
pub mod compression;
#[cfg(any(feature = "encoding", feature = "compression"))]
mod cpu_features;
#[cfg(feature = "encoding")]
pub mod encoding;
