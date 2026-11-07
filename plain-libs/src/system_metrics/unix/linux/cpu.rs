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

// ARM IDs from sysinfo 0.32.1, sourced from https://github.com/util-linux/util-linux/blob/7076703b529d255600631306419cca1b48ab850a/sys-utils/lscpu-arm.c.
fn get_arm_implementer(implementer: u32) -> Option<&'static str> {
    Some(match implementer {
        0x41 => "ARM",
        0x42 => "Broadcom",
        0x43 => "Cavium",
        0x44 => "DEC",
        0x46 => "FUJITSU",
        0x48 => "HiSilicon",
        0x49 => "Infineon",
        0x4d => "Motorola/Freescale",
        0x4e => "NVIDIA",
        0x50 => "APM",
        0x51 => "Qualcomm",
        0x53 => "Samsung",
        0x56 => "Marvell",
        0x61 => "Apple",
        0x66 => "Faraday",
        0x69 => "Intel",
        0x70 => "Phytium",
        0xc0 => "Ampere",
        _ => return None,
    })
}

fn get_arm_part(implementer: u32, part: u32) -> Option<&'static str> {
    Some(match (implementer, part) {
        // ARM
        (0x41, 0x810) => "ARM810",
        (0x41, 0x920) => "ARM920",
        (0x41, 0x922) => "ARM922",
        (0x41, 0x926) => "ARM926",
        (0x41, 0x940) => "ARM940",
        (0x41, 0x946) => "ARM946",
        (0x41, 0x966) => "ARM966",
        (0x41, 0xa20) => "ARM1020",
        (0x41, 0xa22) => "ARM1022",
        (0x41, 0xa26) => "ARM1026",
        (0x41, 0xb02) => "ARM11 MPCore",
        (0x41, 0xb36) => "ARM1136",
        (0x41, 0xb56) => "ARM1156",
        (0x41, 0xb76) => "ARM1176",
        (0x41, 0xc05) => "Cortex-A5",
        (0x41, 0xc07) => "Cortex-A7",
        (0x41, 0xc08) => "Cortex-A8",
        (0x41, 0xc09) => "Cortex-A9",
        (0x41, 0xc0d) => "Cortex-A17", // Originally A12
        (0x41, 0xc0f) => "Cortex-A15",
        (0x41, 0xc0e) => "Cortex-A17",
        (0x41, 0xc14) => "Cortex-R4",
        (0x41, 0xc15) => "Cortex-R5",
        (0x41, 0xc17) => "Cortex-R7",
        (0x41, 0xc18) => "Cortex-R8",
        (0x41, 0xc20) => "Cortex-M0",
        (0x41, 0xc21) => "Cortex-M1",
        (0x41, 0xc23) => "Cortex-M3",
        (0x41, 0xc24) => "Cortex-M4",
        (0x41, 0xc27) => "Cortex-M7",
        (0x41, 0xc60) => "Cortex-M0+",
        (0x41, 0xd01) => "Cortex-A32",
        (0x41, 0xd02) => "Cortex-A34",
        (0x41, 0xd03) => "Cortex-A53",
        (0x41, 0xd04) => "Cortex-A35",
        (0x41, 0xd05) => "Cortex-A55",
        (0x41, 0xd06) => "Cortex-A65",
        (0x41, 0xd07) => "Cortex-A57",
        (0x41, 0xd08) => "Cortex-A72",
        (0x41, 0xd09) => "Cortex-A73",
        (0x41, 0xd0a) => "Cortex-A75",
        (0x41, 0xd0b) => "Cortex-A76",
        (0x41, 0xd0c) => "Neoverse-N1",
        (0x41, 0xd0d) => "Cortex-A77",
        (0x41, 0xd0e) => "Cortex-A76AE",
        (0x41, 0xd13) => "Cortex-R52",
        (0x41, 0xd20) => "Cortex-M23",
        (0x41, 0xd21) => "Cortex-M33",
        (0x41, 0xd40) => "Neoverse-V1",
        (0x41, 0xd41) => "Cortex-A78",
        (0x41, 0xd42) => "Cortex-A78AE",
        (0x41, 0xd43) => "Cortex-A65AE",
        (0x41, 0xd44) => "Cortex-X1",
        (0x41, 0xd46) => "Cortex-A510",
        (0x41, 0xd47) => "Cortex-A710",
        (0x41, 0xd48) => "Cortex-X2",
        (0x41, 0xd49) => "Neoverse-N2",
        (0x41, 0xd4a) => "Neoverse-E1",
        (0x41, 0xd4b) => "Cortex-A78C",
        (0x41, 0xd4c) => "Cortex-X1C",
        (0x41, 0xd4d) => "Cortex-A715",
        (0x41, 0xd4e) => "Cortex-X3",

        // Broadcom
        (0x42, 0x00f) => "Brahma-B15",
        (0x42, 0x100) => "Brahma-B53",
        (0x42, 0x516) => "ThunderX2",

        // Cavium
        (0x43, 0x0a0) => "ThunderX",
        (0x43, 0x0a1) => "ThunderX-88XX",
        (0x43, 0x0a2) => "ThunderX-81XX",
        (0x43, 0x0a3) => "ThunderX-83XX",
        (0x43, 0x0af) => "ThunderX2-99xx",

        // DEC
        (0x44, 0xa10) => "SA110",
        (0x44, 0xa11) => "SA1100",

        // Fujitsu
        (0x46, 0x001) => "A64FX",

        // HiSilicon
        (0x48, 0xd01) => "Kunpeng-920", // aka tsv110

        // NVIDIA
        (0x4e, 0x000) => "Denver",
        (0x4e, 0x003) => "Denver 2",
        (0x4e, 0x004) => "Carmel",

        // APM
        (0x50, 0x000) => "X-Gene",

        // Qualcomm
        (0x51, 0x00f) => "Scorpion",
        (0x51, 0x02d) => "Scorpion",
        (0x51, 0x04d) => "Krait",
        (0x51, 0x06f) => "Krait",
        (0x51, 0x201) => "Kryo",
        (0x51, 0x205) => "Kryo",
        (0x51, 0x211) => "Kryo",
        (0x51, 0x800) => "Falkor-V1/Kryo",
        (0x51, 0x801) => "Kryo-V2",
        (0x51, 0x802) => "Kryo-3XX-Gold",
        (0x51, 0x803) => "Kryo-3XX-Silver",
        (0x51, 0x804) => "Kryo-4XX-Gold",
        (0x51, 0x805) => "Kryo-4XX-Silver",
        (0x51, 0xc00) => "Falkor",
        (0x51, 0xc01) => "Saphira",

        // Samsung
        (0x53, 0x001) => "exynos-m1",

        // Marvell
        (0x56, 0x131) => "Feroceon-88FR131",
        (0x56, 0x581) => "PJ4/PJ4b",
        (0x56, 0x584) => "PJ4B-MP",

        // Apple
        (0x61, 0x020) => "Icestorm-A14",
        (0x61, 0x021) => "Firestorm-A14",
        (0x61, 0x022) => "Icestorm-M1",
        (0x61, 0x023) => "Firestorm-M1",
        (0x61, 0x024) => "Icestorm-M1-Pro",
        (0x61, 0x025) => "Firestorm-M1-Pro",
        (0x61, 0x028) => "Icestorm-M1-Max",
        (0x61, 0x029) => "Firestorm-M1-Max",
        (0x61, 0x030) => "Blizzard-A15",
        (0x61, 0x031) => "Avalanche-A15",
        (0x61, 0x032) => "Blizzard-M2",
        (0x61, 0x033) => "Avalanche-M2",

        // Faraday
        (0x66, 0x526) => "FA526",
        (0x66, 0x626) => "FA626",

        // Intel
        (0x69, 0x200) => "i80200",
        (0x69, 0x210) => "PXA250A",
        (0x69, 0x212) => "PXA210A",
        (0x69, 0x242) => "i80321-400",
        (0x69, 0x243) => "i80321-600",
        (0x69, 0x290) => "PXA250B/PXA26x",
        (0x69, 0x292) => "PXA210B",
        (0x69, 0x2c2) => "i80321-400-B0",
        (0x69, 0x2c3) => "i80321-600-B0",
        (0x69, 0x2d0) => "PXA250C/PXA255/PXA26x",
        (0x69, 0x2d2) => "PXA210C",
        (0x69, 0x411) => "PXA27x",
        (0x69, 0x41c) => "IPX425-533",
        (0x69, 0x41d) => "IPX425-400",
        (0x69, 0x41f) => "IPX425-266",
        (0x69, 0x682) => "PXA32x",
        (0x69, 0x683) => "PXA930/PXA935",
        (0x69, 0x688) => "PXA30x",
        (0x69, 0x689) => "PXA31x",
        (0x69, 0xb11) => "SA1110",
        (0x69, 0xc12) => "IPX1200",

        // Phytium
        (0x70, 0x660) => "FTC660",
        (0x70, 0x661) => "FTC661",
        (0x70, 0x662) => "FTC662",
        (0x70, 0x663) => "FTC663",

        _ => return None,
    })
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
