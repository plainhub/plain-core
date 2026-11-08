// Take a look at the license at the top of the repository in the LICENSE file.

use crate::platform::windows::metrics::{
    FormattedCounter, PDH_FMT_DOUBLE, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterValue, PdhOpenQueryA, PdhRemoveCounter,
};
use std::ptr::{null, null_mut};

pub(crate) struct Query {
    handle: isize,
    counter: isize,
}

impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            if self.counter != 0 {
                PdhRemoveCounter(self.counter);
            }
            PdhCloseQuery(self.handle);
        }
    }
}

impl Query {
    pub(crate) fn new() -> Option<Self> {
        let mut handle = 0;
        if unsafe { PdhOpenQueryA(null(), 0, &mut handle) } != 0 {
            sysinfo_debug!("Cannot open CPU performance query");
            return None;
        }
        let mut query = Self { handle, counter: 0 };
        let path: Vec<_> = r"\Processor(_Total)\% Idle Time"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut counter = 0;
        if unsafe { PdhAddEnglishCounterW(handle, path.as_ptr(), 0, &mut counter) } != 0 {
            sysinfo_debug!("Cannot add CPU idle counter");
            return None;
        }
        query.counter = counter;
        Some(query)
    }

    pub(crate) fn sample_idle(&self) -> Option<f32> {
        if unsafe { PdhCollectQueryData(self.handle) } != 0 {
            sysinfo_debug!("Cannot collect CPU performance data");
            return None;
        }
        let mut value = FormattedCounter::default();
        if unsafe {
            PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, null_mut(), &mut value)
        } != 0
        {
            sysinfo_debug!("Cannot format CPU idle counter");
            return None;
        }
        counter_value(&value)
    }
}

// PDH_CSTATUS_VALID_DATA (0) and PDH_CSTATUS_NEW_DATA (1) are the two successful data statuses.
fn counter_value(value: &FormattedCounter) -> Option<f32> {
    if !matches!(value.status, 0 | 1) {
        sysinfo_debug!("Invalid CPU idle counter status: {}", value.status);
        return None;
    }
    Some(value.double as f32)
}

pub(crate) struct CpusWrapper {
    pub(crate) global: f32,
    brand: Option<String>,
}

impl CpusWrapper {
    pub fn new() -> Self {
        Self {
            global: 0.0,
            brand: None,
        }
    }

    pub fn global_cpu_usage(&self) -> f32 {
        self.global
    }

    pub(crate) fn cpu_model(&self) -> &str {
        self.brand.as_deref().unwrap_or_default()
    }

    pub(crate) fn initialize(&mut self) {
        if self.brand.is_none() {
            self.brand = Some(read_cpu_model());
        }
    }
}

fn read_cpu_model() -> String {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::__cpuid;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::__cpuid;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        cpu_brand(|leaf| {
            let data = __cpuid(leaf);
            [data.eax, data.ebx, data.ecx, data.edx]
        })
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    String::new()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64", test))]
#[forbid(unsafe_code)]
fn cpu_brand(mut cpuid: impl FnMut(u32) -> [u32; 4]) -> String {
    if cpuid(0x80000000)[0] < 0x80000004 {
        return String::new();
    }
    let mut bytes = Vec::with_capacity(48);
    for leaf in 0x80000002..=0x80000004 {
        for word in cpuid(leaf) {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    bytes.truncate(end);
    String::from_utf8(bytes).unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/system_metrics/windows/cpu.rs"]
mod tests;
