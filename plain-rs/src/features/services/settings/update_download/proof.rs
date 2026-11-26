use super::{Identity, validate_url};
use futures_util::StreamExt;
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::crypto::{hash::sha256_reader, release::package_hash};
use plain_libs::protocols::http_client;
use std::io::Read;
use std::path::Path;

const REPOSITORY: &str = "plainhub/plain-app";
const MANIFEST: &str = "release-manifest.txt";
const SIGNATURE: &str = "release-manifest.sig";
const MAX_MANIFEST: u64 = 16 * 1024;
const SIGNATURE_LEN: u64 = 64;

pub(super) struct Asset {
    pub url: http_client::Url,
    base: String,
    filename: String,
}

impl Asset {
    pub fn for_update(identity: &Identity) -> Result<Self> {
        ensure!(
            identity.version.split('.').count() == 3
                && identity.version.split('.').all(|part| {
                    !part.is_empty()
                        && (part == "0" || !part.starts_with('0'))
                        && part.bytes().all(|byte| byte.is_ascii_digit())
                }),
            "Invalid update version"
        );
        let base = format!(
            "https://github.com/{REPOSITORY}/releases/download/v{}/",
            identity.version
        );
        let filename = identity
            .url
            .strip_prefix(&base)
            .ok_or_else(|| anyhow!("Invalid update download URL"))?;
        let prefix = format!("PlainApp-{}-", identity.version);
        ensure!(
            matches!(
                filename.strip_prefix(&prefix),
                Some("64bit-Recommended.apk" | "Old-32bit.apk")
            ),
            "Invalid update package name"
        );
        Ok(Self {
            url: validate_url(&identity.url)?,
            base,
            filename: filename.to_owned(),
        })
    }

    pub async fn download_proof(&self, client: &http_client::Client) -> Result<Proof> {
        let manifest = download(client, &format!("{}{MANIFEST}", self.base), MAX_MANIFEST).await?;
        let signature =
            download(client, &format!("{}{SIGNATURE}", self.base), SIGNATURE_LEN).await?;
        Ok(Proof {
            manifest,
            signature,
        })
    }
}

async fn download(client: &http_client::Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut response = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes_stream();
    let mut contents = Vec::new();
    while let Some(chunk) = response.next().await {
        let chunk = chunk?;
        ensure!(
            (contents.len() as u64).saturating_add(chunk.len() as u64) <= limit,
            "Release proof file exceeds its size limit"
        );
        contents.extend_from_slice(&chunk);
    }
    Ok(contents)
}

pub(super) struct Proof {
    manifest: Vec<u8>,
    signature: Vec<u8>,
}

pub(super) struct VerifiedRelease {
    proof: Proof,
    digest: String,
}

impl Proof {
    pub fn verify(self, identity: &Identity, key: &str) -> Result<VerifiedRelease> {
        let asset = Asset::for_update(identity)?;
        let digest = package_hash(
            &self.manifest,
            &self.signature,
            key,
            REPOSITORY,
            &identity.version,
            &asset.filename,
        )?
        .to_owned();
        Ok(VerifiedRelease {
            proof: self,
            digest,
        })
    }

    pub fn read(directory: &Path) -> Result<Self> {
        Ok(Self {
            manifest: read_bounded(&directory.join(MANIFEST), MAX_MANIFEST)?,
            signature: read_bounded(&directory.join(SIGNATURE), SIGNATURE_LEN)?,
        })
    }
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit,
        "Invalid cached release proof file"
    );
    let mut contents = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut contents)?;
    ensure!(
        contents.len() as u64 <= limit,
        "Invalid cached release proof size"
    );
    Ok(contents)
}

impl VerifiedRelease {
    pub fn verify_package(&self, path: &Path) -> Result<()> {
        let file = std::fs::File::open(path)?;
        ensure!(
            sha256_reader(file)? == self.digest,
            "APK checksum does not match signed release manifest"
        );
        Ok(())
    }

    pub fn write_proof(&self, directory: &Path) -> Result<()> {
        std::fs::write(directory.join(MANIFEST), &self.proof.manifest)?;
        std::fs::write(directory.join(SIGNATURE), &self.proof.signature)?;
        Ok(())
    }
}

#[cfg(any(test, feature = "test-seams"))]
pub(super) fn fixture(identity: &Identity, bytes: &[u8]) -> (Proof, String) {
    use plain_libs::crypto::{ed25519_keypair_from_seed, ed25519_sign};
    use plain_libs::utils::base64::{base64_decode, base64_encode};

    let pair = ed25519_keypair_from_seed(&[73; 32]);
    let mut der = b"\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00".to_vec();
    der.extend_from_slice(&pair[32..]);
    let asset = Asset::for_update(identity).unwrap();
    let manifest = format!(
        "version {}\nrepository {REPOSITORY}\n{}  {}\n",
        identity.version,
        sha256_reader(bytes).unwrap(),
        asset.filename
    )
    .into_bytes();
    let signature = base64_decode(&ed25519_sign(&pair, &manifest));
    (
        Proof {
            manifest,
            signature,
        },
        base64_encode(&der),
    )
}
