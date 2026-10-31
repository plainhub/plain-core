#[cfg(target_os = "linux")]
pub use plain_rs::platform::linux::disks::list::Disk;

#[cfg(not(target_os = "linux"))]
pub struct Disk {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: i64,
    pub removable: bool,
    pub model: Option<String>,
}

pub fn list() -> Result<Vec<Disk>, String> {
    #[cfg(target_os = "linux")]
    {
        plain_rs::platform::linux::disks::list::read_disks()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(sysinfo::Disks::new_with_refreshed_list()
            .iter()
            .map(|disk| Disk {
                id: disk.mount_point().to_string_lossy().into_owned(),
                name: disk.name().to_string_lossy().into_owned(),
                path: disk.mount_point().to_string_lossy().into_owned(),
                size_bytes: disk.total_space().min(i64::MAX as u64) as i64,
                removable: disk.is_removable(),
                model: None,
            })
            .collect())
    }
}
