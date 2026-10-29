use std::{io, path::Path};

pub fn key(path: &Path) -> io::Result<Option<String>> {
    let identity = sample(path, true)?;
    Ok(identity
        .created
        .map(|created| format!("{}:{}:{created}", identity.volume, identity.file)))
}

pub fn same(left: &Path, right: &Path) -> io::Result<bool> {
    let left = sample(left, true)?;
    let right = sample(right, true)?;
    Ok(left.volume == right.volume && left.file == right.file)
}

pub fn within_volume_key(path: &Path) -> io::Result<Option<String>> {
    let identity = sample(path, true)?;
    Ok(identity
        .created
        .map(|created| format!("{}:{created}", identity.file)))
}

pub fn within_volume_entry_key(path: &Path) -> io::Result<Option<String>> {
    let identity = sample(path, false)?;
    Ok(identity
        .created
        .map(|created| format!("{}:{created}", identity.file)))
}

struct Identity {
    volume: String,
    file: String,
    created: Option<String>,
}

#[cfg(unix)]
fn sample(path: &Path, follow: bool) -> io::Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = if follow {
        std::fs::metadata(path)?
    } else {
        std::fs::symlink_metadata(path)?
    };
    let created = metadata
        .created()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|time| time.as_nanos().to_string());
    Ok(Identity {
        volume: metadata.dev().to_string(),
        file: metadata.ino().to_string(),
        created,
    })
}

#[cfg(windows)]
fn sample(path: &Path, follow: bool) -> io::Result<Identity> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    #[repr(C)]
    #[derive(Default)]
    struct Information {
        attributes: u32,
        created: [u32; 2],
        accessed: [u32; 2],
        written: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(handle: *mut std::ffi::c_void, info: *mut Information)
        -> i32;
    }
    let file = std::fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(7)
        .custom_flags(0x02000000 | if follow { 0 } else { 0x00200000 })
        .open(path)?;
    let mut info = Information::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Identity {
        volume: info.volume.to_string(),
        file: format!("{}:{}", info.index_high, info.index_low),
        created: Some(format!("{}:{}", info.created[1], info.created[0])),
    })
}

#[cfg(not(any(unix, windows)))]
fn sample(_path: &Path, _follow: bool) -> io::Result<Identity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "filesystem identity unavailable",
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/utils/file_identity.rs"]
mod tests;
