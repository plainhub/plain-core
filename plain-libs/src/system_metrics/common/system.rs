// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

use crate::system_metrics::SystemInner;

pub struct System {
    inner: SystemInner,
}
impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}
impl System {
    pub fn new() -> Self {
        Self {
            inner: SystemInner::new(),
        }
    }
    pub fn refresh_memory(&mut self) {
        self.inner.refresh_memory()
    }

    pub fn refresh_cpu_usage(&mut self) {
        self.inner.refresh_cpu(true)
    }

    pub fn refresh_cpu_model(&mut self) {
        self.inner.refresh_cpu(false)
    }

    pub fn global_cpu_usage(&self) -> f32 {
        self.inner.global_cpu_usage()
    }

    pub fn cpu_model(&self) -> Option<&str> {
        let brand = self.inner.cpu_model();
        (!brand.is_empty()).then_some(brand)
    }

    pub fn total_memory(&self) -> u64 {
        self.inner.total_memory()
    }

    pub fn available_memory(&self) -> u64 {
        self.inner.available_memory()
    }

    pub fn uptime() -> u64 {
        SystemInner::uptime()
    }

    pub fn name() -> Option<String> {
        SystemInner::name()
    }

    pub fn kernel_version() -> Option<String> {
        SystemInner::kernel_version()
    }

    pub fn os_version() -> Option<String> {
        SystemInner::os_version()
    }
}
