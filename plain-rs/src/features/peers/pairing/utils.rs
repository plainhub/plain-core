use std::time::{SystemTime, UNIX_EPOCH};

const MAX_TIMESTAMP_DIFF_MS: i64 = 5 * 60 * 1000;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn timestamp_ok(ts: i64) -> bool {
    now_ms().abs_diff(ts) <= MAX_TIMESTAMP_DIFF_MS as u64
}

pub fn prefer_sender_ip(ips: &[String], sender_ip: &str) -> String {
    if sender_ip.is_empty() {
        ips.iter()
            .find(|ip| !ip.is_empty())
            .cloned()
            .unwrap_or_default()
    } else {
        sender_ip.to_owned()
    }
}

/// Private (RFC 1918) non-loopback IPv4 addresses of this host, for the
/// pairing request `ips` field.
pub fn local_ipv4_strs() -> Vec<String> {
    plain_libs::utils::ifaddr::list()
        .into_iter()
        .map(|iface| iface.ip)
        .filter(|ip| !ip.is_loopback() && !ip.is_link_local() && ip.is_private())
        .map(|ip| ip.to_string())
        .collect()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/peers/pairing/utils.rs"]
mod tests;
