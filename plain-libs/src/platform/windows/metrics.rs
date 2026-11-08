// Win32 ABI declarations adapted from windows 0.57.0 (https://github.com/microsoft/windows-rs), see LICENSE-MIT.

use std::ffi::c_void;

pub(crate) type Handle = *mut c_void;
#[cfg(feature = "disk-metrics")]
pub(crate) const MAX_PATH: usize = 260;
#[cfg(feature = "disk-metrics")]
pub(crate) const ERROR_NO_MORE_FILES: i32 = 18;
pub(crate) const ERROR_MORE_DATA: i32 = 234;
#[cfg(feature = "disk-metrics")]
pub(crate) const DRIVE_FIXED: u32 = 3;
#[cfg(feature = "disk-metrics")]
pub(crate) const DRIVE_REMOVABLE: u32 = 2;

#[cfg(feature = "disk-metrics")]
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    pub(crate) fn FindFirstVolumeW(name: *mut u16, length: u32) -> Handle;
    pub(crate) fn FindNextVolumeW(search: Handle, name: *mut u16, length: u32) -> i32;
    pub(crate) fn FindVolumeClose(search: Handle) -> i32;
    pub(crate) fn GetDiskFreeSpaceExW(
        path: *const u16,
        available_to_caller: *mut u64,
        total: *mut u64,
        available: *mut u64,
    ) -> i32;
    pub(crate) fn GetDriveTypeW(path: *const u16) -> u32;
    pub(crate) fn GetVolumeInformationW(
        path: *const u16,
        name: *mut u16,
        name_length: u32,
        serial: *mut u32,
        maximum_component_length: *mut u32,
        flags: *mut u32,
        filesystem: *mut u16,
        filesystem_length: u32,
    ) -> i32;
    pub(crate) fn GetVolumePathNamesForVolumeNameW(
        name: *const u16,
        paths: *mut u16,
        paths_length: u32,
        length: *mut u32,
    ) -> i32;
}

#[cfg(feature = "system-metrics")]
pub(crate) const HKEY_LOCAL_MACHINE: Handle = -2147483646isize as Handle;
#[cfg(feature = "system-metrics")]
pub(crate) const KEY_READ: u32 = 131097;
#[cfg(feature = "system-metrics")]
pub(crate) const REG_SZ: u32 = 1;
#[cfg(feature = "system-metrics")]
pub(crate) const REG_EXPAND_SZ: u32 = 2;
#[cfg(feature = "system-metrics")]
pub(crate) const REG_DWORD: u32 = 4;
#[cfg(feature = "system-metrics")]
pub(crate) const PDH_FMT_DOUBLE: u32 = 512;

#[cfg(feature = "system-metrics")]
#[repr(C)]
#[derive(Default)]
pub(crate) struct MemoryStatus {
    pub(crate) length: u32,
    memory_load: u32,
    pub(crate) total_physical: u64,
    pub(crate) available_physical: u64,
    total_page_file: u64,
    available_page_file: u64,
    total_virtual: u64,
    available_virtual: u64,
    available_extended_virtual: u64,
}

#[cfg(feature = "system-metrics")]
#[repr(C)]
#[derive(Default)]
pub(crate) struct FormattedCounter {
    pub(crate) status: u32,
    pub(crate) double: f64,
}

#[cfg(feature = "system-metrics")]
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    pub(crate) fn GetTickCount64() -> u64;
    pub(crate) fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
}

#[cfg(feature = "system-metrics")]
#[link(name = "advapi32", kind = "raw-dylib")]
unsafe extern "system" {
    pub(crate) fn RegCloseKey(key: Handle) -> i32;
    pub(crate) fn RegOpenKeyExW(
        key: Handle,
        path: *const u16,
        options: u32,
        access: u32,
        result: *mut Handle,
    ) -> i32;
    pub(crate) fn RegQueryValueExW(
        key: Handle,
        name: *const u16,
        reserved: *const u32,
        kind: *mut u32,
        data: *mut u8,
        size: *mut u32,
    ) -> i32;
}

#[cfg(feature = "system-metrics")]
#[link(name = "pdh", kind = "raw-dylib")]
unsafe extern "system" {
    pub(crate) fn PdhOpenQueryA(source: *const u8, user_data: usize, query: *mut isize) -> u32;
    pub(crate) fn PdhAddEnglishCounterW(
        query: isize,
        path: *const u16,
        user_data: usize,
        counter: *mut isize,
    ) -> u32;
    pub(crate) fn PdhCloseQuery(query: isize) -> u32;
    pub(crate) fn PdhCollectQueryData(query: isize) -> u32;
    pub(crate) fn PdhGetFormattedCounterValue(
        counter: isize,
        format: u32,
        kind: *mut u32,
        value: *mut FormattedCounter,
    ) -> u32;
    pub(crate) fn PdhRemoveCounter(counter: isize) -> u32;
}
