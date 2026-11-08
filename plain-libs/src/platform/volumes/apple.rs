use super::*;
use crate::platform::apple_device::{
    CF_URL_POSIX_PATH_STYLE, CFURLCopyFileSystemPath, OwnedCF as Owned, cf_file_url,
};
use std::ffi::{CStr, c_void};
use std::os::unix::ffi::OsStrExt;

type Ref = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFURLVolumeUUIDStringKey: Ref;
    static kCFURLVolumeURLKey: Ref;
    static kCFURLCanonicalPathKey: Ref;
    fn CFURLCopyResourcePropertyForKey(url: Ref, key: Ref, value: *mut Ref, error: *mut Ref) -> u8;
}

fn property(url: Ref, key: Ref) -> Result<Option<Owned>> {
    let mut value = std::ptr::null();
    let success =
        unsafe { CFURLCopyResourcePropertyForKey(url, key, &mut value, std::ptr::null_mut()) };
    ensure!(success != 0, "Cannot read native volume property");
    Ok(unsafe { Owned::from_owned(value) })
}

fn resource_url(path: &Path) -> Result<Owned> {
    cf_file_url(path).context("Cannot create filesystem resource URL")
}

pub(super) fn containing(path: &Path) -> Result<Location> {
    let bytes = path.as_os_str().as_bytes();
    let url = resource_url(path)?;
    let root = property(url.as_ptr(), unsafe { kCFURLVolumeURLKey })?
        .context("Volume root is unavailable")?;
    let raw_path = unsafe { CFURLCopyFileSystemPath(root.as_ptr(), CF_URL_POSIX_PATH_STYLE) };
    let root_path =
        unsafe { Owned::from_owned(raw_path) }.context("Cannot resolve volume root path")?;
    let root_path = PathBuf::from(
        unsafe { crate::platform::apple_device::cf_string(root_path.as_ptr()) }
            .context("Cannot decode native volume root")?,
    );
    let uuid = property(url.as_ptr(), unsafe { kCFURLVolumeUUIDStringKey })?;
    let identity = if let Some(uuid) = uuid {
        format!(
            "UUID:{}",
            unsafe { crate::platform::apple_device::cf_string(uuid.as_ptr()) }
                .context("Cannot decode native volume identity")?
                .to_uppercase()
        )
    } else {
        let path = std::ffi::CString::new(bytes)?;
        let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
        ensure!(
            unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) } == 0,
            "Cannot read filesystem volume information"
        );
        let info = unsafe { info.assume_init() };
        let fs_type = unsafe { CStr::from_ptr(info.f_fstypename.as_ptr()) }.to_str()?;
        ensure!(
            matches!(fs_type, "smbfs" | "cifs"),
            "Filesystem has no reliable volume identity"
        );
        share_identity(unsafe { CStr::from_ptr(info.f_mntfromname.as_ptr()) }.to_str()?)?
    };
    let canonical = property(url.as_ptr(), unsafe { kCFURLCanonicalPathKey })?
        .context("Canonical filesystem path is unavailable")?;
    let canonical = PathBuf::from(
        unsafe { crate::platform::apple_device::cf_string(canonical.as_ptr()) }
            .context("Cannot decode canonical volume path")?,
    );
    let root_path = root_for_path(&canonical, root_path, &identity, |root| {
        let url = resource_url(root)?;
        let uuid = property(url.as_ptr(), unsafe { kCFURLVolumeUUIDStringKey })?
            .context("Namespace root volume identity is unavailable")?;
        Ok(format!(
            "UUID:{}",
            unsafe { crate::platform::apple_device::cf_string(uuid.as_ptr()) }
                .context("Cannot decode native volume identity")?
                .to_uppercase()
        ))
    })?;
    Ok(Location {
        volume: Volume {
            identity,
            path: root_path,
        },
        path: canonical,
    })
}

fn root_for_path(
    path: &Path,
    reported: PathBuf,
    identity: &str,
    read_identity: impl FnOnce(&Path) -> Result<String>,
) -> Result<PathBuf> {
    if path.starts_with(&reported) {
        return Ok(reported);
    }
    let root = Path::new("/");
    ensure!(
        read_identity(root)? == identity,
        "File is outside its volume root"
    );
    Ok(root.to_owned())
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/platform/volumes/apple.rs"]
mod tests;
