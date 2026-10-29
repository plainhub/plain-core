use super::*;
use std::ffi::{CStr, c_void};
use std::os::unix::ffi::OsStrExt;

type Ref = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFURLVolumeUUIDStringKey: Ref;
    static kCFURLVolumeURLKey: Ref;
    static kCFURLCanonicalPathKey: Ref;
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: Ref,
        bytes: *const u8,
        len: isize,
        directory: u8,
    ) -> Ref;
    fn CFURLCopyResourcePropertyForKey(url: Ref, key: Ref, value: *mut Ref, error: *mut Ref) -> u8;
    fn CFURLCopyFileSystemPath(url: Ref, style: isize) -> Ref;
    fn CFStringGetLength(value: Ref) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(value: Ref, buffer: *mut i8, size: isize, encoding: u32) -> u8;
    fn CFRelease(value: Ref);
}

struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

fn property(url: Ref, key: Ref) -> Result<Option<Owned>> {
    let mut value = std::ptr::null();
    let success =
        unsafe { CFURLCopyResourcePropertyForKey(url, key, &mut value, std::ptr::null_mut()) };
    ensure!(success != 0, "Cannot read native volume property");
    Ok((!value.is_null()).then(|| Owned(value)))
}

fn string(value: Ref) -> Result<String> {
    const UTF8: u32 = 0x08000100;
    let size = unsafe { CFStringGetMaximumSizeForEncoding(CFStringGetLength(value), UTF8) };
    ensure!(size >= 0, "Invalid native volume string length");
    let mut bytes = vec![0u8; size as usize + 1];
    let copied =
        unsafe { CFStringGetCString(value, bytes.as_mut_ptr().cast(), bytes.len() as isize, UTF8) };
    ensure!(copied != 0, "Cannot decode native volume string");
    Ok(unsafe { CStr::from_ptr(bytes.as_ptr().cast()) }
        .to_str()?
        .to_owned())
}

pub(super) fn containing(path: &Path) -> Result<Location> {
    let bytes = path.as_os_str().as_bytes();
    let raw = unsafe {
        CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            0,
        )
    };
    ensure!(!raw.is_null(), "Cannot create filesystem resource URL");
    let url = Owned(raw);
    let root =
        property(url.0, unsafe { kCFURLVolumeURLKey })?.context("Volume root is unavailable")?;
    let raw_path = unsafe { CFURLCopyFileSystemPath(root.0, 0) };
    ensure!(!raw_path.is_null(), "Cannot resolve volume root path");
    let root_path = Owned(raw_path);
    let root_path = PathBuf::from(string(root_path.0)?);
    let uuid = property(url.0, unsafe { kCFURLVolumeUUIDStringKey })?;
    let identity = if let Some(uuid) = uuid {
        format!("UUID:{}", string(uuid.0)?.to_uppercase())
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
    let canonical = property(url.0, unsafe { kCFURLCanonicalPathKey })?
        .context("Canonical filesystem path is unavailable")?;
    Ok(Location {
        volume: Volume {
            identity,
            path: root_path,
        },
        path: PathBuf::from(string(canonical.0)?),
    })
}
