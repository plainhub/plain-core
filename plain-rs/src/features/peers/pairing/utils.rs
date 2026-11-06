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

/// Candidate-IP list for a paired peer: the socket sender IP first, then
/// the advertised IPs (deduplicated, sender excluded).
pub fn prefer_sender_ip(ips: &[String], sender_ip: &str) -> String {
    let mut all = Vec::with_capacity(ips.len() + 1);
    if !sender_ip.is_empty() {
        all.push(sender_ip.to_string());
    }
    for ip in ips {
        if !ip.is_empty() && ip != sender_ip && !all.contains(ip) {
            all.push(ip.clone());
        }
    }
    all.join(",")
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
