//! Nearby-device discovery stack: LAN service discovery (mDNS browser +
//! responder), peer online/offline state, the Apple dns_sd FFI browser,
//! and Windows mDNS firewall diagnostics.

pub mod firewall;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple_dns_sd;

pub use firewall::MdnsFirewallStatus;

/// One mDNS debug-activity entry (mirrors plain-app's MdnsDebugPage feed).
#[derive(Clone, Debug, serde::Serialize)]
pub struct MdnsActivity {
    pub time: u64,
    pub event: String,
    pub detail: String,
}
