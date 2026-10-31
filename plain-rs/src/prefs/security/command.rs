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
    SetRotateUrlToken {
        enabled: bool,
    },
    ResetPassword,
    ResetUrlToken,
    ResetAdbToken,
    SetApiPermission {
        permission: String,
        enabled: bool,
    },
    SetApiPermissions {
        permissions: Vec<String>,
    },
    PatchApiPermissions {
        permissions: std::collections::BTreeMap<String, bool>,
    },
    SetMdnsHostname {
        hostname: String,
    },
}

impl Command {
    pub fn apply(mut self, prefs: &Prefs) -> anyhow::Result<bool> {
        if let Self::SetMdnsHostname { hostname } = &mut self {
            *hostname = hostname.trim().to_owned();
        }
        match self {
            Self::SetMdnsHostname { ref hostname } => validate_hostname(hostname)?,
            Self::SetApiPermission { ref permission, .. } => anyhow::ensure!(
                PERMISSIONS.contains(&permission.as_str()),
                "Unknown permission"
            ),
            Self::SetApiPermissions { ref permissions } => anyhow::ensure!(
                permissions
                    .iter()
                    .all(|p| PERMISSIONS.contains(&p.as_str())),
                "Unknown permission"
            ),
            Self::PatchApiPermissions { ref permissions } => anyhow::ensure!(
                permissions
                    .keys()
                    .all(|permission| PERMISSIONS.contains(&permission.as_str())),
                "Unknown permission"
            ),
            _ => {}
        }
        let mut changed = false;
        prefs.update(|map| {
            let before = map.clone();
            match self {
                Self::SetPassword { password } => put(map, PASSWORD, password)?,
                Self::SetPasswordType { value } => put(map, PASSWORD_TYPE, value)?,
                Self::SetLoginRequireApproval { enabled } => {
                    put(map, LOGIN_REQUIRE_APPROVAL, enabled)?
                }
                Self::SetRotateUrlToken { enabled } => {
                    put(map, ROTATE_URL_TOKEN_ON_RESTART, enabled)?
                }
                Self::ResetPassword => put(map, PASSWORD, random_password(6))?,
                Self::ResetUrlToken => put(map, URL_TOKEN, crate::gen_token())?,
                Self::ResetAdbToken => put(map, ADB_TOKEN, random_password(32))?,
                Self::SetApiPermission {
                    permission,
                    enabled,
                } => {
                    let mut permissions: Vec<String> = read(map, API_PERMISSIONS)?;
                    permissions.retain(|p| p != &permission);
                    if enabled {
                        permissions.push(permission);
                    }
                    permissions.sort();
                    put(map, API_PERMISSIONS, permissions)?;
                }
                Self::SetApiPermissions { mut permissions } => {
                    permissions.sort();
                    permissions.dedup();
                    put(map, API_PERMISSIONS, permissions)?;
                }
                Self::PatchApiPermissions { permissions } => {
                    let mut current: Vec<String> = read(map, API_PERMISSIONS)?;
                    current.retain(|permission| !permissions.contains_key(permission));
                    current.extend(
                        permissions
                            .into_iter()
                            .filter_map(|(permission, enabled)| enabled.then_some(permission)),
                    );
                    current.sort();
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

fn validate_hostname(hostname: &str) -> anyhow::Result<()> {
    let label = hostname
        .strip_suffix(".local")
        .ok_or_else(|| anyhow::anyhow!("Hostname must end in .local"))?;
    anyhow::ensure!(
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
