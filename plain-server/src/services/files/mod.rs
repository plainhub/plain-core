#[cfg(feature = "core")]
pub mod file_access;

#[cfg(feature = "core")]
pub(crate) mod media_index;

#[cfg(feature = "core")]
pub mod file_task_media;

#[cfg(feature = "core")]
pub mod file_task_walk;

#[cfg(feature = "core")]
pub mod file_tasks;

#[cfg(feature = "core")]
pub mod media_buckets;

#[cfg(feature = "core")]
pub mod writes;

#[cfg(feature = "core")]
pub mod media_actions;

#[cfg(feature = "core")]
pub mod upload_chunks;

#[cfg(feature = "core")]
pub mod archives;

#[cfg(feature = "core")]
pub(crate) mod catalog;

#[cfg(feature = "core")]
pub(crate) mod media_catalog;

#[cfg(feature = "core")]
pub(crate) mod media_catalog_facts;

#[cfg(all(feature = "core", feature = "media"))]
pub mod media_scans;
#[cfg(feature = "media")]
pub mod media_watch;

#[cfg(all(feature = "core", feature = "media"))]
pub(crate) mod media_scan_policy;

#[cfg(any(feature = "core", feature = "mounts"))]
pub mod mounts;

#[cfg(feature = "core")]
pub(crate) mod zip_entries;

#[cfg(feature = "core")]
pub(crate) mod platform;

#[cfg(all(feature = "core", feature = "media"))]
pub(crate) mod native_scan;

#[cfg(feature = "core")]
pub(crate) mod archive_listing;

#[cfg(feature = "core")]
pub(crate) mod browse;

#[cfg(feature = "core")]
pub(crate) mod scanning;

#[cfg(feature = "core")]
pub(crate) mod indexed;

#[cfg(feature = "core")]
pub(crate) mod archive_operations;

#[cfg(feature = "core")]
pub(crate) mod media_provider;

#[cfg(feature = "core")]
pub mod media_action_io;

#[cfg(feature = "core")]
pub(crate) mod resources;

#[cfg(feature = "core")]
pub(crate) mod duration_updates;

#[cfg(feature = "core")]
pub mod browser;

#[cfg(feature = "core")]
pub(crate) mod input;

#[cfg(feature = "core")]
pub(crate) mod remote_cache;

#[cfg(feature = "core")]
pub(crate) mod info;

#[cfg(feature = "core")]
pub(crate) mod references;

#[cfg(feature = "core")]
pub(crate) mod imports;

#[cfg(feature = "core")]
pub(crate) mod source_files;

#[cfg(feature = "core")]
pub(crate) mod note_images;

#[cfg(feature = "core")]
pub(crate) mod catalog_events;

#[cfg(feature = "core")]
pub(crate) mod named_copy;

#[cfg(feature = "core")]
pub(crate) mod external_index;

#[cfg(feature = "core")]
mod watch_backend;

#[cfg(feature = "core")]
pub(crate) mod links;

#[cfg(feature = "core")]
pub(crate) mod content;

#[cfg(feature = "core")]
pub(crate) mod previews;
