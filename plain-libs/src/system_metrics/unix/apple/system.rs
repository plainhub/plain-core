// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

use crate::system_metrics::sys::cpu::CpusWrapper;
use libc::{_SC_PAGESIZE, c_int, host_statistics64, sysconf, sysctl, timeval, vm_statistics64};
use std::mem;
use std::time::Duration;

pub const MINIMUM_CPU_UPDATE_INTERVAL: Duration = Duration::from_millis(200);

fn boot_time() -> u64 {
    let mut boot_time = timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut len = std::mem::size_of::<timeval>();
    let mut mib: [c_int; 2] = [libc::CTL_KERN, libc::KERN_BOOTTIME];

    unsafe {
        if sysctl(
            mib.as_mut_ptr(),
            mib.len() as _,
            &mut boot_time as *mut timeval as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) < 0
        {
            0
        } else {
            boot_time.tv_sec as _
        }
    }
}

fn get_system_info(value: c_int, default: Option<&str>) -> Option<String> {
    let mut mib: [c_int; 2] = [libc::CTL_KERN, value];
    let mut size = 0;

    unsafe {
        // Call first to get size
        sysctl(
            mib.as_mut_ptr(),
            mib.len() as _,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        );

        // exit early if we did not update the size
        if size == 0 {
            default.map(|s| s.to_owned())
        } else {
            // set the buffer to the correct size
            let mut buf = vec![0_u8; size as _];

            if sysctl(
                mib.as_mut_ptr(),
                mib.len() as _,
                buf.as_mut_ptr() as _,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) == -1
            {
                // If command fails return default
                default.map(|s| s.to_owned())
            } else {
                if let Some(pos) = buf.iter().position(|x| *x == 0) {
                    // Shrink buffer to terminate the null bytes
                    buf.resize(pos, 0);
                }

                String::from_utf8(buf).ok()
            }
        }
    }
}

pub(crate) struct SystemInner {
    mem_total: u64,

    mem_available: u64,

    page_size_b: u64,
    port: crate::platform::apple_device::HostPort,
    cpus: CpusWrapper,
}

impl SystemInner {
    pub(crate) fn new() -> Self {
        unsafe {
            let port = crate::platform::apple_device::HostPort::new();

            Self {
                mem_total: 0,

                mem_available: 0,

                page_size_b: sysconf(_SC_PAGESIZE) as _,
                port,
                cpus: CpusWrapper::new(),
            }
        }
    }

    pub(crate) fn refresh_memory(&mut self) {
        if self.mem_total == 0 {
            if let Ok(bytes) = crate::platform::apple_device::memory_bytes() {
                self.mem_total = bytes;
            }
        }
        let mut count = libc::HOST_VM_INFO64_COUNT as _;
        let mut stat = unsafe { mem::zeroed::<vm_statistics64>() };
        if unsafe {
            host_statistics64(
                self.port.get(),
                libc::HOST_VM_INFO64,
                &mut stat as *mut vm_statistics64 as *mut _,
                &mut count,
            )
        } == libc::KERN_SUCCESS
        {
            self.mem_available = u64::from(stat.free_count)
                .saturating_add(u64::from(stat.inactive_count))
                .saturating_add(u64::from(stat.purgeable_count))
                .saturating_sub(u64::from(stat.compressor_page_count))
                .saturating_mul(self.page_size_b);
        }
    }

    pub(crate) fn refresh_cpu(&mut self, sample_usage: bool) {
        self.cpus.refresh(sample_usage);
    }

    pub(crate) fn global_cpu_usage(&self) -> f32 {
        self.cpus.global_usage
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
        unsafe {
            let csec = libc::time(::std::ptr::null_mut());

            libc::difftime(csec, boot_time() as _) as _
        }
    }

    pub(crate) fn name() -> Option<String> {
        get_system_info(libc::KERN_OSTYPE, Some("Darwin"))
    }

    pub(crate) fn kernel_version() -> Option<String> {
        get_system_info(libc::KERN_OSRELEASE, None)
    }

    pub(crate) fn os_version() -> Option<String> {
        crate::platform::apple_device::sysctl_string(c"kern.osproductversion")
    }
}
