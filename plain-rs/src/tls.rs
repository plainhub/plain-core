//! Self-signed TLS certificate management for the local HTTPS servers.
//!
//! Shared by plain-desktop (`local/tls.rs`) and the server (`cmd/run.rs`):
//! load `cert.pem` / `key.pem` from disk if both exist, otherwise generate
//! a self-signed certificate via `rcgen` and persist it. PEM bytes are
//! returned so each project can build its own TLS acceptor (desktop:
//! tokio-rustls, nas: axum-server / rustls) — the acceptor wiring is
//! deliberately NOT shared.

use std::io;
use std::path::Path;

use rcgen::{CertifiedKey, generate_simple_self_signed};

/// Ensure a self-signed certificate + key exist at `cert_path` / `key_path`.
/// If both files exist they are returned as-is; otherwise a new self-signed
/// certificate (EC P-256, per rcgen default) with the given subject alt
/// names is generated and written. Returns `(cert_pem, key_pem)` bytes.
///
/// `san_names` entries may be DNS names (`"localhost"`) or IP addresses
/// (`"127.0.0.1"`) — rcgen detects IPs automatically.
pub fn ensure_self_signed_pem(
    cert_path: &Path,
    key_path: &Path,
    san_names: &[String],
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    if cert_path.exists() && key_path.exists() {
        let cert_pem = std::fs::read(cert_path)?;
        let key_pem = std::fs::read(key_path)?;
        log::info!("tls: loaded existing cert from {}", cert_path.display());
        return Ok((cert_pem, key_pem));
    }

    log::info!(
        "tls: generating new self-signed certificate in {}",
        cert_path.display()
    );
    // Create the parent dir when the cert path carries one (tests use
    // bare file names in flat temp dirs).
    let dir = cert_path.parent().filter(|d| !d.as_os_str().is_empty());
    if let Some(dir) = dir {
        std::fs::create_dir_all(dir)?;
    }

    let (cert_pem, key_pem) = generate_pem(san_names)?;

    std::fs::write(cert_path, &cert_pem)?;
    std::fs::write(key_path, &key_pem)?;
    log::info!("tls: certificate written to {}", cert_path.display());

    Ok((cert_pem, key_pem))
}

pub fn generate_pem(san_names: &[String]) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(san_names.to_vec()).map_err(io::Error::other)?;
    Ok((
        cert.pem().into_bytes(),
        key_pair.serialize_pem().into_bytes(),
    ))
}

pub fn decode_pkcs12(bytes: &[u8], password: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let (chain, key) = pkcs12::decode(bytes, password)?;
    let pem = |label: &str, bytes: &[u8]| {
        let encoded = crate::utils::base64::base64_encode(bytes);
        let mut text = format!("-----BEGIN {label}-----\n");
        for line in encoded.as_bytes().chunks(64) {
            text.push_str(std::str::from_utf8(line).unwrap());
            text.push('\n');
        }
        text.push_str(&format!("-----END {label}-----\n"));
        text
    };
    let certificates = chain
        .iter()
        .map(|cert| pem("CERTIFICATE", cert))
        .collect::<String>();
    Ok((
        certificates.into_bytes(),
        pem("PRIVATE KEY", &key).into_bytes(),
    ))
}

mod pkcs12;

#[cfg(test)]
#[path = "../tests/unit/tls.rs"]
mod tests;
