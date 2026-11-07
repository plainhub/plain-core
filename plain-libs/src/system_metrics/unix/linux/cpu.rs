// Take a look at the license at the top of the repository in the LICENSE file.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::time::Instant;

use crate::system_metrics::sys::utils::to_u64;

pub(crate) struct CpusWrapper {
    pub(crate) global_cpu: CpuUsage,
    pub(crate) brand: Option<String>,
    last_update: Option<Instant>,
}
impl CpusWrapper {
    pub(crate) fn new() -> Self {
        Self {
            global_cpu: CpuUsage::default(),
            brand: None,
            last_update: None,
        }
    }
    pub(crate) fn refresh(&mut self, sample_usage: bool) {
        if !sample_usage {
            if self.brand.is_none() {
                self.brand = Some(
                    get_vendor_id_and_brand()
                        .into_iter()
                        .min_by_key(|(index, _)| *index)
                        .map(|(_, (_, brand))| brand)
                        .unwrap_or_default(),
                );
            }
            return;
        }

        if self.last_update.is_some_and(|last| {
            last.elapsed() <= crate::system_metrics::MINIMUM_CPU_UPDATE_INTERVAL
        }) {
            return;
        }
        self.last_update = Some(Instant::now());
        let file = match File::open("/proc/stat") {
            Ok(file) => file,
            Err(error) => {
                log::debug!("Cannot read CPU counters: {error}");
                return;
            }
        };
        let Some(Ok(line)) = BufReader::new(file).split(b'\n').next() else {
            return;
        };
        if !line.starts_with(b"cpu ") {
            return;
        }
        let mut values = line
            .split(|byte| *byte == b' ')
            .filter(|value| !value.is_empty())
            .skip(1);
        self.global_cpu.set(
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
            values.next().map(to_u64).unwrap_or(0),
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CpuValues {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    guest: u64,
    guest_nice: u64,
}

impl CpuValues {
    pub fn set(
        &mut self,
        user: u64,
        nice: u64,
        system: u64,
        idle: u64,
        iowait: u64,
        irq: u64,
        softirq: u64,
        steal: u64,
        guest: u64,
        guest_nice: u64,
    ) {
        // `guest` is already accounted in `user`.
        self.user = user.saturating_sub(guest);
        // `guest_nice` is already accounted in `nice`.
        self.nice = nice.saturating_sub(guest_nice);
        self.system = system;
        self.idle = idle;
        self.iowait = iowait;
        self.irq = irq;
        self.softirq = softirq;
        self.steal = steal;
        self.guest = guest;
        self.guest_nice = guest_nice;
    }

    pub fn work_time(&self) -> u64 {
        self.user
            .saturating_add(self.nice)
            .saturating_add(self.system)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
    }

    pub fn total_time(&self) -> u64 {
        self.work_time()
            .saturating_add(self.idle)
            .saturating_add(self.iowait)
            // `steal`, `guest` and `guest_nice` are only used if we want to account the "guest"
            // into the computation.
            .saturating_add(self.guest)
            .saturating_add(self.guest_nice)
            .saturating_add(self.steal)
    }
}

#[derive(Default)]
pub(crate) struct CpuUsage {
    percent: f32,
    old_values: CpuValues,
    new_values: CpuValues,
    total_time: u64,
    old_total_time: u64,
}

impl CpuUsage {
    pub(crate) fn set(
        &mut self,
        user: u64,
        nice: u64,
        system: u64,
        idle: u64,
        iowait: u64,
        irq: u64,
        softirq: u64,
        steal: u64,
        guest: u64,
        guest_nice: u64,
    ) {
        macro_rules! min {
            ($a:expr, $b:expr, $def:expr) => {
                if $a > $b { ($a - $b) as f32 } else { $def }
            };
        }
        self.old_values = self.new_values;
        self.new_values.set(
            user, nice, system, idle, iowait, irq, softirq, steal, guest, guest_nice,
        );
        self.total_time = self.new_values.total_time();
        self.old_total_time = self.old_values.total_time();
        self.percent = min!(self.new_values.work_time(), self.old_values.work_time(), 0.)
            / min!(self.total_time, self.old_total_time, 1.)
            * 100.;
        if self.percent > 100. {
            self.percent = 100.; // to prevent the percentage to go above 100%
        }
    }

    pub(crate) fn usage(&self) -> f32 {
        self.percent
    }
}

pub(crate) fn get_vendor_id_and_brand() -> HashMap<usize, (String, String)> {
    let mut s = String::new();
    if File::open("/proc/cpuinfo")
        .and_then(|mut f| f.read_to_string(&mut s))
        .is_err()
    {
        return HashMap::new();
    }

    fn get_value(s: &str) -> String {
        s.split(':')
            .last()
            .map(|x| x.trim().to_owned())
            .unwrap_or_default()
    }

    fn get_hex_value(s: &str) -> u32 {
        s.split(':')
            .last()
            .map(|x| x.trim())
            .filter(|x| x.starts_with("0x"))
            .map(|x| u32::from_str_radix(&x[2..], 16).unwrap())
            .unwrap_or_default()
    }

    #[inline]
    fn is_new_processor(line: &str) -> bool {
        line.starts_with("processor\t")
    }

    #[derive(Default)]
    struct CpuInfo {
        index: usize,
        vendor_id: Option<String>,
        brand: Option<String>,
        implementer: Option<u32>,
        part: Option<u32>,
    }

    impl CpuInfo {
        fn has_all_info(&self) -> bool {
            (self.brand.is_some() && self.vendor_id.is_some())
                || (self.implementer.is_some() && self.part.is_some())
        }

        fn convert(mut self) -> (usize, String, String) {
            let (vendor_id, brand) = if let (Some(implementer), Some(part)) =
                (self.implementer.take(), self.part.take())
            {
                let vendor_id = get_arm_implementer(implementer).map(String::from);
                // It's possible to "model name" even with an ARM CPU, so just in case we can't retrieve
                // the brand from "CPU part", we will then use the value from "model name".
                //
                // Example from raspberry pi 3B+:
                //
                // ```
                // model name      : ARMv7 Processor rev 4 (v7l)
                // CPU implementer : 0x41
                // CPU part        : 0xd03
                // ```
                let brand = get_arm_part(implementer, part)
                    .map(String::from)
                    .or_else(|| self.brand.take());
                (vendor_id, brand)
            } else {
                (self.vendor_id.take(), self.brand.take())
            };
            (
                self.index,
                vendor_id.unwrap_or_default(),
                brand.unwrap_or_default(),
            )
        }
    }

    let mut cpus: HashMap<usize, (String, String)> = HashMap::new();
    let mut lines = s.split('\n');
    while let Some(line) = lines.next() {
        if is_new_processor(line) {
            let index = match line
                .split(':')
                .nth(1)
                .and_then(|i| i.trim().parse::<usize>().ok())
            {
                Some(index) => index,
                None => {
                    sysinfo_debug!("Couldn't get processor ID from {line:?}, ignoring this core");
                    continue;
                }
            };

            let mut info = CpuInfo {
                index,
                ..Default::default()
            };

            while let Some(line) = lines.next() {
                if line.starts_with("vendor_id\t") {
                    info.vendor_id = Some(get_value(line));
                } else if line.starts_with("model name\t") {
                    info.brand = Some(get_value(line));
                } else if line.starts_with("CPU implementer\t") {
                    info.implementer = Some(get_hex_value(line));
                } else if line.starts_with("CPU part\t") {
                    info.part = Some(get_hex_value(line));
                } else if info.has_all_info() || is_new_processor(line) {
                    break;
                }
            }
            let (index, vendor_id, brand) = info.convert();
            cpus.insert(index, (vendor_id, brand));
        }
    }
    cpus
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/system_metrics/unix/linux/cpu.rs"]
mod tests;
