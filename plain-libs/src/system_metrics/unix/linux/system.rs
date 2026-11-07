// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

use crate::system_metrics::sys::cpu::CpusWrapper;
use crate::system_metrics::sys::utils::get_all_utf8_data;
#[cfg(target_os = "android")]
use libc::c_char;
use std::time::Duration;
use std::{fs::File, io::Read, path::Path};

pub const MINIMUM_CPU_UPDATE_INTERVAL: Duration = Duration::from_millis(200);

fn parse_table<F>(content: &str, colsep: char, mut callback: F)
where
    F: FnMut(&str, u64),
{
    for line in content.lines() {
        let mut fields = line.split(colsep);
        let Some(key) = fields.next() else {
            continue;
        };
        let Some(value) = fields
            .next()
            .and_then(|value| value.trim_start().split(' ').next())
            .and_then(|value| value.parse().ok())
        else {
            continue;
        };
        callback(key, value);
    }
}

#[derive(PartialEq, Eq)]
enum InfoType {
    Name,
    OsVersion,

    DistributionID,
}

#[cfg(not(target_os = "android"))]
fn get_system_info_linux(info: InfoType, path: &Path, fallback_path: &Path) -> Option<String> {
    if let Ok(buf) = File::open(path).and_then(|mut f| {
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;
        Ok(buf)
    }) {
        let info_str = match info {
            InfoType::Name => "NAME=",
            InfoType::OsVersion => "VERSION_ID=",
            InfoType::DistributionID => "ID=",
        };

        for line in buf.lines() {
            if let Some(stripped) = line.strip_prefix(info_str) {
                return Some(stripped.replace('"', ""));
            }
        }
    }

    // Fallback to `/etc/lsb-release` file for systems where VERSION_ID is not included.
    // VERSION_ID is not required in the `/etc/os-release` file
    // per https://www.linux.org/docs/man5/os-release.html
    // If this fails for some reason, fallback to None
    let buf = File::open(fallback_path)
        .and_then(|mut f| {
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;
            Ok(buf)
        })
        .ok()?;

    let info_str = match info {
        InfoType::OsVersion => "DISTRIB_RELEASE=",
        InfoType::Name => "DISTRIB_ID=",
        InfoType::DistributionID => {
            // lsb-release is inconsistent with os-release and unsupported.
            return None;
        }
    };
    for line in buf.lines() {
        if let Some(stripped) = line.strip_prefix(info_str) {
            return Some(stripped.replace('"', ""));
        }
    }
    None
}

#[cfg(target_os = "android")]
fn get_system_info_android(info: InfoType) -> Option<String> {
    // https://android.googlesource.com/platform/frameworks/base/+/refs/heads/master/core/java/android/os/Build.java#58
    let name: &'static [u8] = match info {
        InfoType::Name => b"ro.product.model\0",
        InfoType::OsVersion => b"ro.build.version.release\0",
        InfoType::DistributionID => {
            // Not supported.
            return None;
        }
    };

    let mut value_buffer = vec![0u8; libc::PROP_VALUE_MAX as usize];
    unsafe {
        let len = libc::__system_property_get(
            name.as_ptr() as *const c_char,
            value_buffer.as_mut_ptr() as *mut c_char,
        );

        if len != 0 {
            if let Some(pos) = value_buffer.iter().position(|c| *c == 0) {
                value_buffer.resize(pos, 0);
            }
            String::from_utf8(value_buffer).ok()
        } else {
            None
        }
    }
}

pub(crate) struct SystemInner {
    mem_total: u64,
    mem_free: u64,
    mem_available: u64,
    mem_buffers: u64,
    mem_page_cache: u64,
    mem_shmem: u64,
    mem_slab_reclaimable: u64,

    cpus: CpusWrapper,
}

impl SystemInner {
    pub(crate) fn new() -> Self {
        Self {
            mem_total: 0,
            mem_free: 0,
            mem_available: 0,
            mem_buffers: 0,
            mem_page_cache: 0,
            mem_shmem: 0,
            mem_slab_reclaimable: 0,

            cpus: CpusWrapper::new(),
        }
    }

    pub(crate) fn refresh_memory(&mut self) {
        if let Ok(content) = get_all_utf8_data("/proc/meminfo", 16_635) {
            self.update_memory(&content);
        }
    }

    fn update_memory(&mut self, content: &str) {
        let mut mem_available_found = false;
        parse_table(content, ':', |key, value_kib| {
            let field = match key {
                "MemTotal" => &mut self.mem_total,
                "MemFree" => &mut self.mem_free,
                "MemAvailable" => {
                    mem_available_found = true;
                    &mut self.mem_available
                }
                "Buffers" => &mut self.mem_buffers,
                "Cached" => &mut self.mem_page_cache,
                "Shmem" => &mut self.mem_shmem,
                "SReclaimable" => &mut self.mem_slab_reclaimable,
                _ => return,
            };
            // /proc/meminfo reports KiB, though it says "kB". Convert it.
            *field = value_kib.saturating_mul(1_024);
        });

        // Linux < 3.14 may not have MemAvailable in /proc/meminfo
        // So it should fallback to the old way of estimating available memory
        // https://github.com/KittyKatt/screenFetch/issues/386#issuecomment-249312716
        if !mem_available_found {
            self.mem_available = self
                .mem_free
                .saturating_add(self.mem_buffers)
                .saturating_add(self.mem_page_cache)
                .saturating_add(self.mem_slab_reclaimable)
                .saturating_sub(self.mem_shmem);
        }
    }

    pub(crate) fn refresh_cpu(&mut self, sample_usage: bool) {
        self.cpus.refresh(sample_usage);
    }

    pub(crate) fn global_cpu_usage(&self) -> f32 {
        self.cpus.global_cpu.usage()
    }

    pub(crate) fn cpu_model(&self) -> &str {
        self.cpus.brand.as_deref().unwrap_or_default()
    }

    pub(crate) fn total_memory(&self) -> u64 {
        self.mem_total
    }

    pub(crate) fn available_memory(&self) -> u64 {
        self.mem_available
    }

    pub(crate) fn uptime() -> u64 {
        let content = get_all_utf8_data("/proc/uptime", 50).unwrap_or_default();
        content
            .split('.')
            .next()
            .and_then(|t| t.parse().ok())
            .unwrap_or_default()
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn name() -> Option<String> {
        get_system_info_linux(
            InfoType::Name,
            Path::new("/etc/os-release"),
            Path::new("/etc/lsb-release"),
        )
    }

    pub(crate) fn kernel_version() -> Option<String> {
        let mut raw = std::mem::MaybeUninit::<libc::utsname>::zeroed();

        unsafe {
            if libc::uname(raw.as_mut_ptr()) == 0 {
                let info = raw.assume_init();

                let release = info
                    .release
                    .iter()
                    .filter(|c| **c != 0)
                    .map(|c| *c as u8 as char)
                    .collect::<String>();

                Some(release)
            } else {
                None
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn os_version() -> Option<String> {
        get_system_info_linux(
            InfoType::OsVersion,
            Path::new("/etc/os-release"),
            Path::new("/etc/lsb-release"),
        )
    }
    #[cfg(target_os = "android")]
    pub(crate) fn name() -> Option<String> {
        get_system_info_android(InfoType::Name)
    }
    #[cfg(target_os = "android")]
    pub(crate) fn os_version() -> Option<String> {
        get_system_info_android(InfoType::OsVersion)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/system_metrics/unix/linux/system/test.rs"]
mod tests;
