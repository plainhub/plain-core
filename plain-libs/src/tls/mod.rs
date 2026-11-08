use std::io;
use std::path::Path;

/// Ensure a self-signed certificate + key exist at `cert_path` / `key_path`.
/// If both files exist they are returned as-is; otherwise a new self-signed
/// certificate (EC P-256) with the given subject alt
/// names is generated and written. Returns `(cert_pem, key_pem)` bytes.
///
/// `san_names` entries may be DNS names (`"localhost"`) or IP addresses
/// (`"127.0.0.1"`) — IP addresses are detected automatically.
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
    let (cert, key) = self_signed::generate(san_names)?;
    Ok((
        pem(b"CERTIFICATE", &cert, b"\r\n"),
        pem(b"PRIVATE KEY", key.as_ref(), b"\r\n"),
    ))
}

fn pem(label: &[u8], bytes: &[u8], newline: &[u8]) -> Vec<u8> {
    let encoded = crate::utils::base64::base64_encode(bytes);
    let mut text = Vec::with_capacity(
        30 + 2 * label.len() + encoded.len() + (encoded.len().div_ceil(64) + 2) * newline.len(),
    );
    text.extend_from_slice(b"-----BEGIN ");
    text.extend_from_slice(label);
    text.extend_from_slice(b"-----");
    text.extend_from_slice(newline);
    for line in encoded.as_bytes().chunks(64) {
        text.extend_from_slice(line);
        text.extend_from_slice(newline);
    }
    text.extend_from_slice(b"-----END ");
    text.extend_from_slice(label);
    text.extend_from_slice(b"-----");
    text.extend_from_slice(newline);
    text
}

pub fn decode_pkcs12(bytes: &[u8], password: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let (chain, key) = pkcs12::decode(bytes, password)?;
    let mut certificates = Vec::new();
    for cert in chain {
        certificates.extend(pem(b"CERTIFICATE", &cert, b"\n"));
    }
    Ok((certificates, pem(b"PRIVATE KEY", &key, b"\n")))
}

mod pkcs12;
mod self_signed;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/tls/mod.rs"]
mod tests;
