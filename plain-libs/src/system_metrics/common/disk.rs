use std::{ffi::OsString, path::PathBuf};

pub struct Disk {
    pub name: OsString,
    pub file_system: OsString,
    pub mount_point: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub removable: bool,
}
