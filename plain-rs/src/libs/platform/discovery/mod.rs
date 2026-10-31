//! Nearby-device discovery stack: LAN service discovery (mDNS browser +
//! responder), peer online/offline state, the macOS dns_sd FFI browser,
//! and Windows mDNS firewall diagnostics.

pub mod firewall;
#[cfg(target_os = "macos")]
pub mod macos_dns_sd;

pub use firewall::MdnsFirewallStatus;

/// One mDNS debug-activity entry (mirrors plain-app's MdnsDebugPage feed).
#[derive(Clone, Debug, serde::Serialize)]
pub struct MdnsActivity {
    pub time: u64,
    pub event: String,
    pub detail: String,
}
