//! URL builder that omits default scheme ports (80/443).

pub fn build_url(scheme: &str, host: &str, port: u16, path: &str) -> String {
    let omit_port = ((scheme == "http" || scheme == "ws") && port == 80)
        || ((scheme == "https" || scheme == "wss") && port == 443);
    let port_part = if omit_port {
        String::new()
    } else {
        format!(":{port}")
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    format!("{scheme}://{host}{port_part}{path}")
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/build_url.rs"]
mod tests;
