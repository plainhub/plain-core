pub const SYSTEM_MOUNT_FACTS: &str = "systemMountFacts";

pub const SYSTEM_SAMBA_SERVICE_FACTS: &str = "systemSambaServiceFacts";
pub const SYSTEM_SAMBA_APPLY: &str = "systemSambaApply";
pub const SYSTEM_SAMBA_PASSWORD: &str = "systemSambaPassword";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SambaServiceFacts {
    pub name: String,
    pub active: bool,
    pub enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SambaApplyRequest {
    pub settings: plain_rs::prefs::samba::SambaSettings,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SambaPasswordRequest {
    pub password: String,
}

pub const SYSTEM_DISK_FORMAT: &str = "systemDiskFormat";
pub const SYSTEM_HOSTNAME_SET: &str = "systemHostnameSet";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskFormatRequest {
    pub path: String,
    pub client_id: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostnameRequest {
    pub name: String,
}
