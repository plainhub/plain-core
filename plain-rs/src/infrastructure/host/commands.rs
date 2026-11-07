pub const SYSTEM_MOUNT_FACTS: &str = "systemMountFacts";

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
