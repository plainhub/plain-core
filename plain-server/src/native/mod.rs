#[cfg(feature = "core")]
pub mod events;

#[cfg(feature = "core")]
pub mod assets;

#[cfg(feature = "core")]
pub mod graphql;

#[cfg(feature = "core")]
pub mod management;

#[cfg(feature = "core")]
pub mod resources;

#[cfg(feature = "core")]
pub mod uploads;

#[cfg(all(feature = "core", feature = "media"))]
pub mod audio;
