use crate::features::access::AccessFeature;
use super::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum Command {
    SetPassword {
        password: String,
    },
    SetPasswordType {
        value: PasswordType,
    },
    SetLoginRequireApproval {
        enabled: bool,
    },
    ResetAdbToken,
    SetApiPermission {
        permission: AccessFeature,
        enabled: bool,
    },
    SetApiPermissions {
        permissions: Vec<AccessFeature>,
    },
    PatchApiPermissions {
        permissions: std::collections::BTreeMap<AccessFeature, bool>,
    },
    SetMdnsHostname {
        hostname: String,
    },
}

impl Command {
    pub fn apply(mut self, prefs: &Prefs) -> plain_libs::anyhow::Result<bool> {
        if let Self::SetMdnsHostname { hostname } = &mut self {
            *hostname = hostname.trim().to_owned();
        }
        match self {
            Self::SetPassword { ref password } => {
                plain_libs::anyhow::ensure!(
                    !password.trim().is_empty() && password.len() <= 1024,
                    "Invalid password"
                );
            }
            Self::SetMdnsHostname { ref hostname } => validate_hostname(hostname)?,
            _ => {}
        }
        let mut changed = false;
        prefs.update(|map| {
            let before = map.clone();
            match self {
                Self::SetPassword { password } => password_login::write_password(map, &password)?,
                Self::SetPasswordType { value } => put(map, PASSWORD_TYPE, value)?,
                Self::SetLoginRequireApproval { enabled } => {
                    put(map, LOGIN_REQUIRE_APPROVAL, enabled)?
                }
                Self::ResetAdbToken => put(map, ADB_TOKEN, random_password(32))?,
                Self::SetApiPermission {
                    permission,
                    enabled,
                } => {
                    let mut permissions: Vec<AccessFeature> = read(map, API_PERMISSIONS)?;
                    permissions.retain(|p| p != &permission);
                    if enabled {
                        permissions.push(permission);
                    }
                    permissions.sort_by_key(|feature| feature.as_str());
                    put(map, API_PERMISSIONS, permissions)?;
                }
                Self::SetApiPermissions { mut permissions } => {
                    permissions.sort_by_key(|feature| feature.as_str());
                    permissions.dedup();
                    put(map, API_PERMISSIONS, permissions)?;
                }
                Self::PatchApiPermissions { permissions } => {
                    let mut current: Vec<AccessFeature> = read(map, API_PERMISSIONS)?;
                    current.retain(|permission| !permissions.contains_key(permission));
                    current.extend(
                        permissions
                            .into_iter()
                            .filter_map(|(permission, enabled)| enabled.then_some(permission)),
                    );
                    current.sort_by_key(|feature| feature.as_str());
                    current.dedup();
                    put(map, API_PERMISSIONS, current)?;
                }
                Self::SetMdnsHostname { hostname } => put(map, MDNS_HOSTNAME, hostname)?,
            }
            changed = before != *map;
            Ok(())
        })?;
        Ok(changed)
    }
}

fn validate_hostname(hostname: &str) -> plain_libs::anyhow::Result<()> {
    let label = hostname
        .strip_suffix(".local")
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Hostname must end in .local"))?;
    plain_libs::anyhow::ensure!(
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Invalid mDNS hostname"
    );
    Ok(())
}
