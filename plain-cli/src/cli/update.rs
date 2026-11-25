use plain_libs::anyhow::Result;
#[cfg(any(not(target_os = "linux"), test))]
use plain_libs::anyhow::bail;
#[cfg(any(target_os = "linux", test))]
use plain_libs::anyhow::ensure;
use plain_rs::infrastructure::platform::system::consts::AppPaths;

#[cfg(target_os = "linux")]
pub fn run(paths: &AppPaths) -> Result<()> {
    linux::run(paths)
}

#[cfg(not(target_os = "linux"))]
pub fn run(_paths: &AppPaths) -> Result<()> {
    bail!("System service updates are available on Linux")
}

#[cfg(any(target_os = "linux", test))]
fn version(value: &str) -> Result<[u64; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    ensure!(parts.len() == 3, "Invalid release version");
    let mut result = [0; 3];
    for (number, part) in result.iter_mut().zip(parts) {
        ensure!(
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0')),
            "Invalid release version"
        );
        *number = part.parse()?;
    }
    Ok(result)
}

#[cfg(any(target_os = "linux", test))]
fn archive_hash<'a>(
    manifest: &'a [u8],
    signature: &[u8],
    public_key: &str,
    release: &str,
    schema: &str,
    archive: &str,
) -> Result<&'a str> {
    use plain_libs::crypto::ed25519_verify;
    use plain_libs::utils::base64::{base64_decode, base64_encode};

    let der = base64_decode(public_key);
    let ed25519_spki = b"\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00";
    let Some(key) = der.strip_prefix(ed25519_spki.as_slice()) else {
        plain_libs::anyhow::bail!("Invalid release signing public key");
    };
    ensure!(
        ed25519_verify(&base64_encode(key), manifest, &base64_encode(signature)),
        "Release manifest signature verification failed"
    );
    let manifest = std::str::from_utf8(manifest)?;
    let mut lines = manifest.lines();
    ensure!(
        lines.next().and_then(|line| line.strip_prefix("version ")) == Some(release),
        "Release manifest version does not match the release tag"
    );
    ensure!(
        lines.next().and_then(|line| line.strip_prefix("schema ")) == Some(schema),
        "Release changes the database schema. Use the release's manual upgrade instructions"
    );
    let mut selected = None;
    for name in [
        "plainapp-linux-amd64.tar.gz",
        "plainapp-linux-arm64.tar.gz",
        "install.sh",
    ] {
        let Some((hash, filename)) = lines.next().and_then(|line| line.split_once("  ")) else {
            plain_libs::anyhow::bail!("Invalid release manifest");
        };
        ensure!(
            filename == name
                && hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "Invalid release manifest"
        );
        if name == archive {
            selected = Some(hash);
        }
    }
    ensure!(lines.next().is_none(), "Invalid release manifest");
    selected.ok_or_else(|| plain_libs::anyhow::anyhow!("Unsupported release archive"))
}

#[cfg(target_os = "linux")]
#[path = "update_linux.rs"]
mod linux;

#[cfg(test)]
#[path = "../../../tests/plain-cli/unit/cli/update.rs"]
mod tests;
