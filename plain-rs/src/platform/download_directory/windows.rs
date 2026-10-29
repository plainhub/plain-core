use std::{ffi::c_void, os::windows::ffi::OsStringExt, path::PathBuf};

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        folder: *const Guid,
        flags: u32,
        token: *mut c_void,
        path: *mut *mut u16,
    ) -> i32;
}
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(memory: *mut c_void);
}

pub(super) fn resolve() -> Option<PathBuf> {
    let downloads = Guid {
        data1: 0x374de290,
        data2: 0x123f,
        data3: 0x4565,
        data4: [0x91, 0x64, 0x39, 0xc4, 0x92, 0x5e, 0x46, 0x7b],
    };
    let mut path = std::ptr::null_mut();
    let status = unsafe { SHGetKnownFolderPath(&downloads, 0, std::ptr::null_mut(), &mut path) };
    let result = unsafe { result_path(status, path) };
    unsafe { CoTaskMemFree(path.cast()) };
    result
}

unsafe fn result_path(status: i32, path: *const u16) -> Option<PathBuf> {
    if status >= 0 && !path.is_null() {
        let mut length = 0;
        while unsafe { *path.add(length) } != 0 {
            length += 1;
        }
        Some(PathBuf::from(std::ffi::OsString::from_wide(unsafe {
            std::slice::from_raw_parts(path, length)
        })))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/download_directory/windows.rs"]
mod tests;
