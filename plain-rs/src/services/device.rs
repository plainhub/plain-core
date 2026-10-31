#[cfg(any(target_os = "ios", all(test, target_os = "macos")))]
mod apple;
pub mod info;
mod native_battery;
pub mod permissions;

use plain_rs::services::device::types::{DeviceStatus, Temperature};

use plain_rs::host::bridge::Host;
use plain_rs::types::Long;
use serde::Deserialize;
use std::{sync::Arc, time::Instant};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct Status {
    directory: std::path::PathBuf,
    fixed_info: Option<Arc<Result<plain_rs::services::device::types::DeviceInfo, String>>>,
    cpu: Arc<Mutex<Option<(Instant, f64)>>>,
}

#[derive(Default, Clone, Copy)]
pub struct Selection {
    pub uptime: bool,
    pub battery: bool,
    pub temperatures: bool,
    pub cpu: bool,
    pub memory: bool,
    pub storage: bool,
}

impl Selection {
    fn direct_fields(self) -> bool {
        self.uptime
            || (self.battery && !cfg!(any(target_os = "ios", target_os = "android")))
            || self.storage
            || (self.memory && !cfg!(target_os = "ios"))
            || (self.temperatures && cfg!(target_os = "linux"))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatterySample {
    level: Option<f64>,
    scale: Option<f64>,
    charging: bool,
    temperature_tenths: Option<i32>,
}

impl BatterySample {
    fn percent(&self) -> Option<i32> {
        let (level, scale) = (self.level?, self.scale?);
        (level.is_finite() && scale.is_finite() && level >= 0.0 && scale > 0.0)
            .then(|| (level * 100.0 / scale).clamp(0.0, 100.0) as i32)
    }
}

impl Status {
    pub fn new(directory: std::path::PathBuf, host: &Host) -> Self {
        Self {
            directory,
            fixed_info: Some(Arc::new(info::startup(host))),
            ..Default::default()
        }
    }
    pub fn fixed_info(&self) -> Result<plain_rs::services::device::types::DeviceInfo, String> {
        self.fixed_info
            .as_ref()
            .ok_or("Device information is not initialized")?
            .as_ref()
            .clone()
    }
    pub fn directory(&self) -> &std::path::Path {
        &self.directory
    }
    async fn sample_cpu(
        &self,
        requested: Instant,
        sample: impl FnOnce() -> f64 + Send + 'static,
    ) -> Result<f64, String> {
        let mut previous = self.cpu.clone().lock_owned().await;
        if let Some((completed, value)) = *previous {
            if completed >= requested {
                return Ok(value);
            }
        }
        tokio::task::spawn_blocking(move || {
            let value = sample();
            *previous = Some((Instant::now(), value));
            value
        })
        .await
        .map_err(|error| error.to_string())
    }

    pub async fn collect(&self, host: &Host, selection: Selection) -> Result<DeviceStatus, String> {
        let requested = Instant::now();
        let cpu = async {
            if selection.cpu {
                self.sample_cpu(requested, cpu_usage).await
            } else {
                Ok(0.0)
            }
        };
        let readings = async {
            if selection.direct_fields() {
                {
                    let directory = self.directory.clone();
                    tokio::task::spawn_blocking(move || direct(selection, &directory))
                }
                .await
                .map_err(|error| error.to_string())?
            } else {
                direct(selection, &self.directory)
            }
        };
        let battery = async {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            if selection.battery || (selection.temperatures && cfg!(target_os = "android")) {
                return serde_json::from_value::<BatterySample>(
                    host.call("systemBatterySample", serde_json::json!({}))
                        .await?,
                )
                .map(Some)
                .map_err(|error| error.to_string());
            }
            let _ = host;
            Ok::<Option<BatterySample>, String>(None)
        };
        let (cpu, readings, battery) = tokio::join!(cpu, readings, battery);
        let mut status = readings?;
        status.cpu_usage = cpu?;
        if let Some(sample) = battery? {
            status.battery_level = sample.percent();
            status.charging = sample.charging;
            if selection.temperatures {
                if let Some(tenths) = sample.temperature_tenths {
                    status.temperatures.push(Temperature {
                        label: "battery".into(),
                        celsius: f64::from(tenths) / 10.0,
                    });
                }
            }
        }
        Ok(status)
    }
}

#[cfg(not(target_os = "ios"))]
fn cpu_usage() -> f64 {
    let mut system = sysinfo::System::new();
    system.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();
    f64::from(system.global_cpu_usage()).clamp(0.0, 100.0)
}

#[cfg(target_os = "ios")]
fn cpu_usage() -> f64 {
    let previous = apple::cpu_ticks();
    std::thread::sleep(std::time::Duration::from_millis(200));
    apple::cpu_usage(previous, apple::cpu_ticks())
}

fn direct(selection: Selection, directory: &std::path::Path) -> Result<DeviceStatus, String> {
    let mut status = DeviceStatus {
        uptime_sec: Long(if selection.uptime { uptime()? } else { 0 }),
        battery_level: None,
        charging: false,
        temperatures: Vec::new(),
        cpu_usage: 0.0,
        memory_available: None,
        storage_available: Long(0),
    };
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if selection.battery {
        (status.battery_level, status.charging) = native_battery::read();
    }
    #[cfg(not(target_os = "ios"))]
    if selection.memory {
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        status.memory_available = Some(Long(system.available_memory().min(i64::MAX as u64) as i64));
    }
    if selection.storage {
        status.storage_available = Long(storage_bytes(directory, false)?);
    }
    #[cfg(target_os = "linux")]
    if selection.temperatures {
        status.temperatures = plain_rs::platform::linux::device_info::thermal_zones(
            std::path::Path::new("/sys/class/thermal"),
        )
        .into_iter()
        .map(|value| Temperature {
            label: value.label,
            celsius: value.celsius,
        })
        .collect();
    }
    Ok(status)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn uptime() -> Result<i64, String> {
    #[cfg(target_os = "android")]
    let clock = libc::CLOCK_BOOTTIME;
    #[cfg(target_os = "ios")]
    let clock = libc::CLOCK_UPTIME_RAW;
    let mut time = std::mem::MaybeUninit::<libc::timespec>::uninit();
    if unsafe { libc::clock_gettime(clock, time.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(unsafe { time.assume_init() }.tv_sec.max(0) as i64)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn uptime() -> Result<i64, String> {
    Ok(sysinfo::System::uptime().min(i64::MAX as u64) as i64)
}

#[cfg(unix)]
fn storage_bytes(directory: &std::path::Path, total: bool) -> Result<i64, String> {
    #[cfg(target_os = "android")]
    let path =
        std::ffi::CString::new(std::env::var("ANDROID_DATA").unwrap_or_else(|_| "/data".into()))
            .map_err(|error| error.to_string())?;
    #[cfg(target_os = "ios")]
    let path = std::ffi::CString::new("/").unwrap();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let path = {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::CString::new(directory.as_os_str().as_bytes())
            .map_err(|error| error.to_string())?
    };
    let _ = directory;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let stats = unsafe { stats.assume_init() };
    Ok((if total {
        stats.f_blocks as u64
    } else {
        stats.f_bavail as u64
    })
    .saturating_mul(stats.f_frsize as u64)
    .min(i64::MAX as u64) as i64)
}

#[cfg(any(not(unix), test))]
fn volume_index<'a>(
    mounts: impl Iterator<Item = &'a std::path::Path>,
    directory: &std::path::Path,
) -> Option<usize> {
    mounts
        .enumerate()
        .filter(|(_, mount)| directory.starts_with(mount))
        .max_by_key(|(_, mount)| mount.as_os_str().len())
        .map(|(index, _)| index)
}

#[cfg(not(unix))]
fn storage_bytes(directory: &std::path::Path, total: bool) -> Result<i64, String> {
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let disk = volume_index(disks.iter().map(|disk| disk.mount_point()), &directory)
        .and_then(|index| disks.list().get(index));
    Ok(disk
        .map(|disk| {
            if total {
                disk.total_space()
            } else {
                disk.available_space()
            }
        })
        .unwrap_or(0)
        .min(i64::MAX as u64) as i64)
}

pub mod permission;
pub mod types;

pub mod access;

#[cfg(test)]
#[path = "../../tests/unit/services/device.rs"]
mod tests;
