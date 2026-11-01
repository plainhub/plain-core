use super::*;
use std::os::windows::ffi::{OsStrExt, OsStringExt};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetVolumePathNameW(file: *const u16, volume: *mut u16, size: u32) -> i32;
    fn GetVolumeNameForVolumeMountPointW(path: *const u16, name: *mut u16, size: u32) -> i32;
}

fn wide(value: &[u16]) -> Result<String> {
    let end = value
        .iter()
        .position(|value| *value == 0)
        .context("Unterminated native volume path")?;
    let value = std::ffi::OsString::from_wide(&value[..end])
        .into_string()
        .map_err(|_| anyhow::anyhow!("Volume path is not Unicode"))?;
    Ok(windows_path(&value))
}

pub(super) fn containing(path: &Path) -> Result<Volume> {
    let encoded = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut root = vec![0u16; 32768];
    if unsafe { GetVolumePathNameW(encoded.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut guid = [0u16; 50];
    let root_path = wide(&root)?;
    let identity = if root_path.starts_with("//") {
        share_identity(&root_path)?
    } else {
        if unsafe {
            GetVolumeNameForVolumeMountPointW(root.as_ptr(), guid.as_mut_ptr(), guid.len() as u32)
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        format!("UUID:{}", wide(&guid)?.to_uppercase())
    };
    Ok(Volume {
        identity,
        path: PathBuf::from(root_path),
    })
}
