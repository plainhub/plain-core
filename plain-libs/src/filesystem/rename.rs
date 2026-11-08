use std::{io, path::Path};

pub use crate::platform::rename::no_replace;

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
