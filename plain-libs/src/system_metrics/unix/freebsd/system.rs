// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

use crate::system_metrics::sys::cpu::CpusWrapper;
use crate::system_metrics::sys::utils::{
    boot_time, get_sys_value, get_sys_value_by_name, init_mib,
};
use libc::c_int;
use std::time::Duration;

pub const MINIMUM_CPU_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

struct SystemInfo {
    hw_physical_memory: [c_int; 2],
    page_size: c_int,
    virtual_page_count: [c_int; 4],
    virtual_cache_count: [c_int; 4],
    virtual_inactive_count: [c_int; 4],
    virtual_free_count: [c_int; 4],
    buf_space: [c_int; 2],
}
impl SystemInfo {
    fn new() -> Self {
        let mut info = Self {
            hw_physical_memory: [0; 2],
            page_size: 0,
            virtual_page_count: [0; 4],
            virtual_cache_count: [0; 4],
            virtual_inactive_count: [0; 4],
            virtual_free_count: [0; 4],
            buf_space: [0; 2],
        };
        unsafe {
            if !get_sys_value_by_name(b"vm.stats.vm.v_page_size\0", &mut info.page_size) {
                panic!("cannot get page size");
            }
            init_mib(b"hw.physmem\0", &mut info.hw_physical_memory);
            init_mib(b"vm.stats.vm.v_page_count\0", &mut info.virtual_page_count);
            init_mib(
                b"vm.stats.vm.v_cache_count\0",
                &mut info.virtual_cache_count,
            );
            init_mib(
                b"vm.stats.vm.v_inactive_count\0",
                &mut info.virtual_inactive_count,
            );
            init_mib(b"vm.stats.vm.v_free_count\0", &mut info.virtual_free_count);
            init_mib(b"vfs.bufspace\0", &mut info.buf_space);
        }
        info
    }
    fn get_total_memory(&self) -> u64 {
        let mut nb_pages: u64 = 0;
        unsafe {
            if get_sys_value(&self.virtual_page_count, &mut nb_pages) {
                return nb_pages.saturating_mul(self.page_size as _);
            }

            // This is a fallback. It includes all the available memory, not just the one available
            // for the users.
            let mut total_memory: u64 = 0;
            get_sys_value(&self.hw_physical_memory, &mut total_memory);
            total_memory
        }
    }
    fn get_free_memory(&self) -> u64 {
        let mut buffers_mem: u64 = 0;
        let mut inactive_mem: u64 = 0;
        let mut cached_mem: u64 = 0;
        let mut free_mem: u64 = 0;

        unsafe {
            get_sys_value(&self.buf_space, &mut buffers_mem);
            get_sys_value(&self.virtual_inactive_count, &mut inactive_mem);
            get_sys_value(&self.virtual_cache_count, &mut cached_mem);
            get_sys_value(&self.virtual_free_count, &mut free_mem);
            // For whatever reason, buffers_mem is already the right value...
            buffers_mem
                .saturating_add(inactive_mem.saturating_mul(self.page_size as _))
                .saturating_add(cached_mem.saturating_mul(self.page_size as _))
                .saturating_add(free_mem.saturating_mul(self.page_size as _))
        }
    }
}

fn get_system_info(mib: &[c_int], default: Option<&str>) -> Option<String> {
    let mut size = 0;

    unsafe {
        // Call first to get size
        libc::sysctl(
            mib.as_ptr(),
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
            let mut buf: Vec<libc::c_char> = vec![0; size as _];

            if libc::sysctl(
                mib.as_ptr(),
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
                crate::system_metrics::sys::utils::c_buf_to_utf8_string(&buf)
            }
        }
    }
}

pub(crate) struct SystemInner {
    mem_total: u64,
    mem_free: u64,

    system_info: SystemInfo,
    cpus: CpusWrapper,
}

impl SystemInner {
    pub(crate) fn new() -> Self {
        Self {
            mem_total: 0,
            mem_free: 0,

            system_info: SystemInfo::new(),
            cpus: CpusWrapper::new(),
        }
    }

    pub(crate) fn refresh_memory(&mut self) {
        if self.mem_total == 0 {
            self.mem_total = self.system_info.get_total_memory();
        }
        self.mem_free = self.system_info.get_free_memory();
    }

    pub(crate) fn refresh_cpu(&mut self, sample_usage: bool) {
        self.cpus.refresh(sample_usage)
    }

    pub(crate) fn global_cpu_usage(&self) -> f32 {
        self.cpus.global_cpu_usage
    }

    pub(crate) fn cpu_model(&self) -> &str {
        ""
    }

    pub(crate) fn total_memory(&self) -> u64 {
        self.mem_total
    }

    pub(crate) fn available_memory(&self) -> u64 {
        self.mem_free
    }

    pub(crate) fn uptime() -> u64 {
        unsafe {
            let csec = libc::time(std::ptr::null_mut());

            libc::difftime(csec, boot_time() as _) as u64
        }
    }

    pub(crate) fn name() -> Option<String> {
        let mut os_type: [c_int; 2] = [0; 2];
        unsafe {
            init_mib(b"kern.ostype\0", &mut os_type);
            get_system_info(&os_type, Some("FreeBSD"))
        }
    }

    pub(crate) fn kernel_version() -> Option<String> {
        let mut kern_version: [c_int; 2] = [0; 2];
        unsafe {
            init_mib(b"kern.version\0", &mut kern_version);
            get_system_info(&kern_version, None)
        }
    }

    pub(crate) fn os_version() -> Option<String> {
        let mut os_release: [c_int; 2] = [0; 2];
        unsafe {
            init_mib(b"kern.osrelease\0", &mut os_release);
            // It returns something like "13.0-RELEASE". We want to keep everything until the "-".
            get_system_info(&os_release, None)
                .and_then(|s| s.split('-').next().map(|s| s.to_owned()))
        }
    }
}
