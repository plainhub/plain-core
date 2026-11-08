use std::{io, path::Path};

#[cfg(unix)]
pub fn no_replace(src: &Path, dst: &Path) -> io::Result<()> {
    let src = std::ffi::CString::new(src.as_os_str().as_encoded_bytes())?;
    let dst = std::ffi::CString::new(dst.as_os_str().as_encoded_bytes())?;
    #[cfg(target_vendor = "apple")]
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            src.as_ptr(),
            libc::AT_FDCWD,
            dst.as_ptr(),
            libc::RENAME_EXCL,
        ) as i64
    };
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            src.as_ptr(),
            libc::AT_FDCWD,
            dst.as_ptr(),
            libc::RENAME_NOREPLACE,
        ) as i64
    };
    #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
    let result = {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace rename unavailable",
        ));
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(windows)]
pub fn no_replace(src: &Path, dst: &Path) -> io::Result<()> {
    move_file_wide(&wide_path(src)?, &wide_path(dst)?, false)
}

#[cfg(windows)]
pub(crate) fn move_file_wide(src: &WidePath, dst: &WidePath, overwrite: bool) -> io::Result<()> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(src: *const u16, dst: *const u16, flags: u32) -> i32;
    }
    // SAFETY: both paths come from wide_path and are NUL-terminated.
    if unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), u32::from(overwrite)) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
pub(crate) struct WidePath(Vec<u16>);

#[cfg(windows)]
impl WidePath {
    pub(crate) fn as_ptr(&self) -> *const u16 {
        self.0.as_ptr()
    }
}

#[cfg(windows)]
pub(crate) fn wide_path(path: &Path) -> io::Result<WidePath> {
    use std::os::windows::ffi::OsStrExt;
    let mut encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if encoded.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    encoded.push(0);
    Ok(WidePath(encoded))
}
