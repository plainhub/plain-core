use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope {
    Session(String),
    Global,
    Share(String),
}

impl Scope {
    fn parts(&self) -> (char, &str) {
        match self {
            Self::Session(id) => ('c', id),
            Self::Global => ('g', ""),
            Self::Share(id) => ('s', id),
        }
    }

    fn aad(&self, purpose: &str) -> Vec<u8> {
        let (mode, owner) = self.parts();
        format!("plain-{purpose}-v1\0{mode}\0{owner}").into_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceType {
    Media,
    AppFile,
    File,
    PackageIcon,
    SharedFile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub r#type: ResourceType,
    pub id: String,
}

impl Resource {
    pub fn reference(path: &str) -> Self {
        let (r#type, id) = if let Some(id) = path.strip_prefix("fid:") {
            (ResourceType::AppFile, id)
        } else if let Some(id) = path.strip_prefix("pkgicon://") {
            (ResourceType::PackageIcon, id)
        } else {
            (ResourceType::File, path)
        };
        Self {
            r#type,
            id: id.into(),
        }
    }

    fn validate(&self, scope: &Scope) -> Result<()> {
        ensure!(!self.id.is_empty(), "Empty resource ID");
        ensure!(
            matches!(scope, Scope::Share(_)) == (self.r#type == ResourceType::SharedFile),
            "Invalid resource scope"
        );
        Ok(())
    }
}

pub fn parse(envelope: &str) -> Result<(Scope, &str)> {
    ensure!(envelope.len() <= 32768, "Resource URL too long");
    let (mode, rest) = envelope
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("Invalid resource envelope"))?;
    let (scope, payload) = match mode {
        "g" => (Scope::Global, rest),
        "c" | "s" => {
            let (owner, payload) = rest
                .rsplit_once('.')
                .ok_or_else(|| anyhow::anyhow!("Invalid resource owner"))?;
            ensure!(
                !owner.is_empty() && owner.len() <= 4096,
                "Invalid resource owner"
            );
            (
                if mode == "c" {
                    Scope::Session(owner.into())
                } else {
                    Scope::Share(owner.into())
                },
                payload,
            )
        }
        _ => anyhow::bail!("Unknown resource mode"),
    };
    ensure!(
        !payload.is_empty()
            && payload
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Invalid resource encoding"
    );
    Ok((scope, payload))
}

pub fn encrypt(scope: &Scope, purpose: &str, key: &[u8], plain: &[u8]) -> Result<String> {
    let bytes = crate::crypto::xchacha_encrypt_raw_with_aad(key, plain, &scope.aad(purpose))
        .ok_or_else(|| anyhow::anyhow!("Invalid resource key"))?;
    let payload = crate::utils::base64::base64_encode_url_safe(&bytes);
    let payload = payload.trim_end_matches('=');
    let (mode, owner) = scope.parts();
    let envelope = if matches!(scope, Scope::Global) {
        format!("{mode}.{payload}")
    } else {
        format!("{mode}.{owner}.{payload}")
    };
    parse(&envelope)?;
    Ok(envelope)
}

pub fn decrypt(scope: &Scope, purpose: &str, key: &[u8], payload: &str) -> Result<Vec<u8>> {
    let bytes = crate::utils::base64::base64_decode_checked(payload)?;
    crate::crypto::xchacha_decrypt_raw_with_aad(key, &bytes, &scope.aad(purpose))
        .ok_or_else(|| anyhow::anyhow!("Invalid resource authentication"))
}

pub fn seal(scope: &Scope, key: &[u8], resource: &Resource) -> Result<String> {
    resource.validate(scope)?;
    encrypt(scope, "fs", key, &serde_json::to_vec(resource)?)
}

pub fn open(scope: &Scope, key: &[u8], payload: &str) -> Result<Resource> {
    let resource: Resource = serde_json::from_slice(&decrypt(scope, "fs", key, payload)?)?;
    resource.validate(scope)?;
    Ok(resource)
}

#[cfg(test)]
#[path = "../../tests/unit/files/resource_link.rs"]
mod tests;
