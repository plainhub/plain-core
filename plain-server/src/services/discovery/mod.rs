#[cfg(feature = "core")]
pub mod local;


#[cfg(feature = "core")]
pub mod ble_pairing;

#[cfg(feature = "core")]
pub mod ble_scan;

#[cfg(feature = "core")]
pub mod discovery_advertisement;

#[cfg(feature = "core")]
pub mod mdns_runtime;

#[cfg(feature = "core")]
pub mod nearby_devices;

#[cfg(feature = "core")]
#[cfg(feature = "core")]
pub mod pairing;

#[cfg(feature = "core")]
pub mod pairing_runtime;

#[cfg(feature = "core")]
pub mod pairing_timeout;

#[cfg(feature = "core")]
pub(crate) mod pairing_attempts;

#[cfg(feature = "core")]
mod pairing_login;

#[cfg(feature = "core")]
pub(crate) mod ble_advertising;

#[cfg(feature = "core")]
pub(crate) mod pairing_confirmation;
