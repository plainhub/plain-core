#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn read() -> (Option<i32>, bool) {
    #[cfg(target_os = "macos")]
    {
        return std::process::Command::new("pmset")
            .args(["-g", "batt"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| parse_pmset_battery(&String::from_utf8_lossy(&output.stdout)))
            .map(|(level, charging)| (Some(level), charging))
            .unwrap_or((None, false));
    }
    #[cfg(target_os = "windows")]
    {
        return win_power::read()
            .map(|(level, charging)| (Some(level), charging))
            .unwrap_or((None, false));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    (None, false)
}

#[cfg(any(target_os = "macos", test))]
fn parse_pmset_battery(out: &str) -> Option<(i32, bool)> {
    if !out.contains("InternalBattery") {
        return None;
    }
    let mut level = None;
    let mut charging = false;
    for line in out.lines() {
        if !line.contains("InternalBattery") {
            continue;
        }
        if let Some(pct_part) = line.split('%').next()
            && let Some(pct_str) = pct_part.split_whitespace().last()
            && let Ok(pct) = pct_str.parse::<i32>()
        {
            level = Some(pct);
        }
        if line.contains("charging") && !line.contains("discharging") {
            charging = true;
        }
    }
    level.map(|l| (l, charging))
}

#[cfg(any(target_os = "windows", test))]
pub fn win_battery_from(battery_flag: u8, battery_life_percent: u8) -> Option<(i32, bool)> {
    if battery_flag & 128 != 0 || battery_life_percent == 255 || battery_life_percent > 100 {
        return None;
    }
    Some((battery_life_percent as i32, battery_flag & 8 != 0))
}

#[cfg(target_os = "windows")]
mod win_power {
    #[repr(C)]
    #[derive(Default)]
    struct SystemPowerStatus {
        ac_line_status: u8,
        battery_flag: u8,
        battery_life_percent: u8,
        reserved1: u8,
        battery_life_time: u32,
        battery_full_life_time: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
    }

    pub fn read() -> Option<(i32, bool)> {
        let mut s = SystemPowerStatus::default();
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return None;
        }
        plain_rs::services::device::win_battery_from(s.battery_flag, s.battery_life_percent)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/device/native_battery.rs"]
mod tests;
