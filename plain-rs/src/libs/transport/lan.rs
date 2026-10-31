use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Interface {
    pub ip: Ipv4Addr,
    pub prefix_length: u8,
}
pub fn local_interfaces() -> Vec<Interface> {
    crate::libs::utils::ifaddr::list()
        .into_iter()
        .filter_map(|interface| {
            if interface.ip.is_unspecified() {
                return None;
            }
            // A mask that is not a run of leading ones means the kernel gave
            // us something we cannot read as a prefix length.
            let mask = u32::from(interface.netmask?);
            let prefix_length = mask.leading_ones() as u8;
            let expected = if prefix_length == 0 {
                0
            } else {
                u32::MAX << (32 - prefix_length)
            };
            (mask == expected).then_some(Interface {
                ip: interface.ip,
                prefix_length,
            })
        })
        .collect()
}
pub fn valid_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
        || (host.len() <= 253
            && host.bytes().any(|b| b.is_ascii_alphabetic())
            && host.trim_end_matches('.').split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            }))
}
pub fn best_host(hosts: &[String], local: &[Interface]) -> String {
    let ip = best(hosts, local);
    if !ip.is_empty() {
        return ip;
    }
    hosts
        .iter()
        .find(|host| valid_host(host))
        .cloned()
        .unwrap_or_default()
}
pub fn best(ips: &[String], local: &[Interface]) -> String {
    let valid: Vec<_> = ips
        .iter()
        .filter_map(|ip| ip.parse::<IpAddr>().ok().map(|parsed| (ip, parsed)))
        .collect();
    for (ip, parsed) in &valid {
        if let IpAddr::V4(address) = parsed {
            if local.iter().any(|interface| {
                if interface.prefix_length > 32 {
                    return false;
                }
                let mask = if interface.prefix_length == 0 {
                    0
                } else {
                    u32::MAX << (32 - interface.prefix_length)
                };
                u32::from(*address) & mask == u32::from(interface.ip) & mask
            }) {
                return (*ip).clone();
            }
        }
    }
    valid
        .iter()
        .find(|(_, parsed)| matches!(parsed,IpAddr::V4(address) if address.is_private()))
        .or(valid.first())
        .map(|(ip, _)| (*ip).clone())
        .unwrap_or_default()
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/transport/lan.rs"]
mod tests;
