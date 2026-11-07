use crate::platform::apple_device;

pub(crate) struct CpusWrapper {
    pub(crate) brand: Option<String>,
    pub(crate) global_usage: f32,
    previous_ticks: Option<[u32; 4]>,
}
impl CpusWrapper {
    pub(crate) fn new() -> Self {
        Self {
            brand: None,
            global_usage: 0.0,
            previous_ticks: None,
        }
    }
    pub(crate) fn refresh(&mut self, sample_usage: bool) {
        if !sample_usage {
            if self.brand.is_none() {
                self.brand = Some(
                    apple_device::sysctl_string(c"machdep.cpu.brand_string").unwrap_or_default(),
                );
            }
            return;
        }

        let current = apple_device::cpu_ticks();
        self.global_usage = apple_device::cpu_usage(self.previous_ticks, current) as f32;
        self.previous_ticks = current;
    }
}
