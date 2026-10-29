use std::net::Ipv4Addr;

pub fn build_m_search(st: &str) -> String {
    let st = st.trim();
    let st_final = if st.is_empty() { "ssdp:all" } else { st };
    format!(
        "M-SEARCH * HTTP/1.1\n\
         HOST: 239.255.255.250:1900\n\
         MAN: \"ssdp:discover\"\n\
         MX: 3\n\
         ST: {st_final}\n\
         \n\
         \n"
    )
}

pub fn local_ipv4_addrs() -> std::collections::HashMap<Ipv4Addr, Option<Ipv4Addr>> {
    let mut out = std::collections::HashMap::new();
    for iface in plain_rs::utils::ifaddr::list() {
        let ip = iface.ip;
        let bcast = iface
            .netmask
            .map(|mask| Ipv4Addr::from(u32::from(ip) | !u32::from(mask)));
        out.insert(ip, bcast);
    }
    out
}

pub fn parse_ssdp_headers(packet: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    // Skip the request/response line. SSDP messages have a single
    // blank line separator; everything before it is the request line +
    // headers. We do case-insensitive header lookup, so we lowercase
    // the keys.
    let mut lines = packet.split('\n');
    let _ = lines.next(); // request line
    for line in lines {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            out.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    out
}

/// Parse the UDN out of a USN header, if present. Mirrors Go
/// `parseUDNFromUSN`. Example: `uuid:4d696e69-444c-164e-9d41-b827abcdef01::upnp:rootdevice`
/// → `uuid:4d696e69-444c-164e-9d41-b827abcdef01`.
pub fn parse_udn_from_usn(usn: &str) -> String {
    let s = usn.trim();
    let stripped = if let Some(rest) = s.strip_prefix("uuid:") {
        rest
    } else {
        s
    };
    let head = stripped.split("::").next().unwrap_or("");
    if head.is_empty() {
        return String::new();
    }
    if head.starts_with("uuid:") {
        head.to_string()
    } else {
        format!("uuid:{head}")
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/casting/sender/ssdp.rs"]
mod tests;
