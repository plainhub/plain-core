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

// https://learn.microsoft.com/windows/win32/shell/knownfolderid
pub enum Folder {
    Downloads,
    Windows,
    ProgramFiles,
    ProgramFilesX86,
    ProgramFilesX64,
    ProgramData,
}

pub fn resolve(folder: Folder) -> Option<PathBuf> {
    let (data1, data2, data3, data4) = match folder {
        Folder::Downloads => (
            0x374de290,
            0x123f,
            0x4565,
            [0x91, 0x64, 0x39, 0xc4, 0x92, 0x5e, 0x46, 0x7b],
        ),
        Folder::Windows => (
            0xf38bf404,
            0x1d43,
            0x42f2,
            [0x93, 0x05, 0x67, 0xde, 0x0b, 0x28, 0xfc, 0x23],
        ),
        Folder::ProgramFiles => (
            0x905e63b6,
            0xc1bf,
            0x494e,
            [0xb2, 0x9c, 0x65, 0xb7, 0x32, 0xd3, 0xd2, 0x1a],
        ),
        Folder::ProgramFilesX86 => (
            0x7c5a40ef,
            0xa0fb,
            0x4bfc,
            [0x87, 0x4a, 0xc0, 0xf2, 0xe0, 0xb9, 0xfa, 0x8e],
        ),
        Folder::ProgramFilesX64 => (
            0x6d809377,
            0x6af0,
            0x444b,
            [0x89, 0x57, 0xa3, 0x77, 0x3f, 0x02, 0x20, 0x0e],
        ),
        Folder::ProgramData => (
            0x62ab5d82,
            0xfdc1,
            0x4dc3,
            [0xa9, 0xdd, 0x07, 0x0d, 0x1d, 0x49, 0x5d, 0x97],
        ),
    };
    let folder = Guid {
        data1,
        data2,
        data3,
        data4,
    };
    let mut path = std::ptr::null_mut();
    let status = unsafe { SHGetKnownFolderPath(&folder, 0, std::ptr::null_mut(), &mut path) };
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
#[path = "../../../../tests/plain-rs/unit/libs/platform/known_folders.rs"]
mod tests;
