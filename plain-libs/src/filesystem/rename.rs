use std::{io, path::Path};

#[cfg(unix)]
pub fn no_replace(src: &Path, dst: &Path) -> io::Result<()> {
    let src = std::ffi::CString::new(src.as_os_str().as_encoded_bytes())?;
    let dst = std::ffi::CString::new(dst.as_os_str().as_encoded_bytes())?;
    #[cfg(target_vendor = "apple")]
    let result = unsafe { libc::renamex_np(src.as_ptr(), dst.as_ptr(), libc::RENAME_EXCL) as i64 };
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
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(src: *const u16, dst: *const u16, flags: u32) -> i32;
    }
    let encode = |path: &Path| -> io::Result<Vec<u16>> {
        let mut encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if encoded.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains NUL",
            ));
        }
        encoded.push(0);
        Ok(encoded)
    };
    let src = encode(src)?;
    let dst = encode(dst)?;
    if unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), 0) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(crate) fn copy_required(error: &io::Error, overwrite: bool) -> bool {
    if error.kind() == io::ErrorKind::CrossesDevices {
        return true;
    }
    #[cfg(unix)]
    if !overwrite
        && error
            .raw_os_error()
            .is_some_and(|code| matches!(code, libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP))
    {
        return true;
    }
    false
}
pub(crate) fn same_identity(src: &Path, dst: &Path) -> io::Result<bool> {
    crate::utils::file_identity::same(src, dst)
}
