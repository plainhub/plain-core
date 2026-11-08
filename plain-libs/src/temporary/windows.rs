use super::Builder;
use crate::platform::rename::{move_file_wide, wide_path};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io,
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
};

const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x100;
const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
const FILE_GENERIC_READ_WRITE: u32 = 0x0012_019f;
const FILE_SHARE_ALL: u32 = 7;
const INVALID_HANDLE_VALUE: *mut c_void = -1_isize as *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn ReOpenFile(file: *mut c_void, access: u32, share: u32, flags: u32) -> *mut c_void;
    fn SetFileAttributesW(path: *const u16, attributes: u32) -> i32;
}

pub(super) fn open_named(path: &Path, options: &mut OpenOptions) -> io::Result<File> {
    options.custom_flags(FILE_ATTRIBUTE_TEMPORARY).open(path)
}

pub(super) fn anonymous(dir: &Path) -> io::Result<File> {
    Builder::new().create(dir, |path| {
        OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .share_mode(0)
            .custom_flags(FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE)
            .open(path)
    })
}

pub(super) fn reopen(file: &File, _: &Path) -> io::Result<File> {
    // SAFETY: the original handle is live and a successful call returns a new owned handle.
    let handle = unsafe {
        ReOpenFile(
            file.as_raw_handle(),
            FILE_GENERIC_READ_WRITE,
            FILE_SHARE_ALL,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_handle(handle) })
    }
}

pub(super) fn persist(source: &Path, destination: &Path, overwrite: bool) -> io::Result<()> {
    let source_wide = wide_path(source)?;
    let destination_wide = wide_path(destination)?;
    // SAFETY: wide_path validates and NUL-terminates the source path.
    if unsafe { SetFileAttributesW(source_wide.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0 {
        return Err(io::Error::last_os_error());
    }
    move_file_wide(&source_wide, &destination_wide, overwrite).inspect_err(|_| {
        unsafe { SetFileAttributesW(source_wide.as_ptr(), FILE_ATTRIBUTE_TEMPORARY) };
    })
}
