// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

use crate::platform::windows::metrics::{
    ERROR_MORE_DATA, GetTickCount64, GlobalMemoryStatusEx, HKEY_LOCAL_MACHINE, Handle, KEY_READ,
    MemoryStatus, REG_DWORD, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use crate::system_metrics::sys::cpu::*;
use std::ptr::{null, null_mut};
use std::time::Duration;
use std::{ffi::OsStr, mem::size_of, os::windows::ffi::OsStrExt};

pub const MINIMUM_CPU_UPDATE_INTERVAL: Duration = Duration::from_millis(200);
const WINDOWS_ELEVEN_BUILD_NUMBER: u32 = 22000;
fn utf16_str<S: AsRef<OsStr> + ?Sized>(text: &S) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>()
}

pub(crate) fn get_reg_string_value(hkey: Handle, path: &str, field_name: &str) -> Option<String> {
    let path = utf16_str(path);
    let field = utf16_str(field_name);
    let key = unsafe { RegKey::open(hkey, &path) }?;
    let mut bytes = vec![0; 2048];
    loop {
        let mut length = u32::try_from(bytes.len()).ok()?;
        match unsafe { key.get_value(&field, &mut bytes, &mut length) } {
            Ok(kind) if kind == REG_SZ || kind == REG_EXPAND_SZ => {
                return registry_string(bytes.get(..length as usize)?);
            }
            Err(error) if error == ERROR_MORE_DATA => {
                bytes.resize(length as usize, 0);
            }
            _ => return None,
        }
    }
}

fn registry_string(bytes: &[u8]) -> Option<String> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return None;
    }
    let words: Vec<_> = pairs.iter().copied().map(u16::from_le_bytes).collect();
    Some(
        String::from_utf16_lossy(&words)
            .trim_end_matches('\0')
            .to_owned(),
    )
}

pub(crate) fn get_reg_value_u32(hkey: Handle, path: &str, field_name: &str) -> Option<[u8; 4]> {
    let path = utf16_str(path);
    let field = utf16_str(field_name);
    let key = unsafe { RegKey::open(hkey, &path) }?;
    let mut length = 4;
    let mut bytes = [0; 4];
    let kind = unsafe { key.get_value(&field, &mut bytes, &mut length) }.ok()?;
    (kind == REG_DWORD && length == 4).then_some(bytes)
}

struct RegKey(Handle);

impl RegKey {
    unsafe fn open(hkey: Handle, path: &[u16]) -> Option<Self> {
        let mut key = null_mut();
        (unsafe { RegOpenKeyExW(hkey, path.as_ptr(), 0, KEY_READ, &mut key) } == 0)
            .then(|| Self(key))
    }

    unsafe fn get_value(
        &self,
        field_name: &[u16],
        buf: &mut [u8],
        buf_len: &mut u32,
    ) -> Result<u32, i32> {
        let mut kind = 0;
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                field_name.as_ptr(),
                null(),
                &mut kind,
                buf.as_mut_ptr(),
                buf_len,
            )
        };
        if status == 0 { Ok(kind) } else { Err(status) }
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        let _err = unsafe { RegCloseKey(self.0) };
    }
}

pub(crate) struct SystemInner {
    mem_total: u64,
    mem_available: u64,

    cpus: CpusWrapper,
    query: Option<Query>,
}

impl SystemInner {
    pub(crate) fn new() -> Self {
        Self {
            mem_total: 0,
            mem_available: 0,

            cpus: CpusWrapper::new(),
            query: None,
        }
    }

    pub(crate) fn refresh_memory(&mut self) {
        let mut info = MemoryStatus::default();
        info.length = size_of::<MemoryStatus>() as _;
        let _result = unsafe { GlobalMemoryStatusEx(&mut info) };
        self.mem_total = info.total_physical;
        self.mem_available = info.available_physical;
    }

    pub(crate) fn refresh_cpu(&mut self, sample_usage: bool) {
        if !sample_usage {
            self.cpus.initialize();
            return;
        }
        if self.query.is_none() {
            self.query = Query::new();
        }
        if let Some(idle) = self.query.as_ref().and_then(Query::sample_idle) {
            self.cpus.global = 100.0 - idle;
        }
    }

    pub(crate) fn global_cpu_usage(&self) -> f32 {
        self.cpus.global_cpu_usage()
    }

    pub(crate) fn cpu_model(&self) -> &str {
        self.cpus.cpu_model()
    }

    pub(crate) fn total_memory(&self) -> u64 {
        self.mem_total
    }

    pub(crate) fn available_memory(&self) -> u64 {
        self.mem_available
    }

    pub(crate) fn uptime() -> u64 {
        unsafe { GetTickCount64() / 1_000 }
    }

    pub(crate) fn name() -> Option<String> {
        Some("Windows".to_owned())
    }

    pub(crate) fn kernel_version() -> Option<String> {
        get_reg_string_value(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        )
    }

    pub(crate) fn os_version() -> Option<String> {
        let build_number = get_reg_string_value(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        )
        .unwrap_or_default();
        let major = if Self::is_windows_eleven() {
            11u32
        } else {
            u32::from_le_bytes(
                get_reg_value_u32(
                    HKEY_LOCAL_MACHINE,
                    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                    "CurrentMajorVersionNumber",
                )
                .unwrap_or_default(),
            )
        };
        Some(format!("{major} ({build_number})"))
    }
    fn is_windows_eleven() -> bool {
        WINDOWS_ELEVEN_BUILD_NUMBER
            <= Self::kernel_version()
                .unwrap_or_default()
                .parse()
                .unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/system_metrics/windows/system.rs"]
mod tests;
