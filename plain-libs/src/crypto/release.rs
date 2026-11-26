use crate::anyhow::{Result, ensure};
use crate::utils::base64::{base64_decode_checked, base64_encode};

pub fn verify_manifest<'a>(
    manifest: &'a [u8],
    signature: &[u8],
    public_key: &str,
) -> Result<&'a str> {
    let der = base64_decode_checked(public_key)?;
    let ed25519_spki = b"\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00";
    let Some(key) = der.strip_prefix(ed25519_spki.as_slice()) else {
        crate::anyhow::bail!("Invalid release signing public key");
    };
    ensure!(
        super::ed25519_verify(&base64_encode(key), manifest, &base64_encode(signature)),
        "Release manifest signature verification failed"
    );
    Ok(std::str::from_utf8(manifest)?)
}

pub fn package_hash<'a>(
    manifest: &'a [u8],
    signature: &[u8],
    public_key: &str,
    repository: &str,
    version: &str,
    package: &str,
) -> Result<&'a str> {
    let mut lines = verify_manifest(manifest, signature, public_key)?.lines();
    ensure!(
        lines.next().and_then(|line| line.strip_prefix("version ")) == Some(version),
        "Release manifest version does not match the release tag"
    );
    ensure!(
        lines
            .next()
            .and_then(|line| line.strip_prefix("repository "))
            == Some(repository),
        "Release manifest does not match the update repository"
    );

    let mut previous = "";
    let mut selected = None;
    for line in lines {
        let (hash, name) = line
            .split_once("  ")
            .ok_or_else(|| crate::anyhow::anyhow!("Invalid release manifest entry"))?;
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                && name > previous,
            "Invalid or duplicate release manifest entry"
        );
        previous = name;
        if name == package {
            selected = Some(hash);
        }
    }
    selected.ok_or_else(|| crate::anyhow::anyhow!("Package not found in signed release manifest"))
}
