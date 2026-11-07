// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.
use super::utils::{get_sys_value_array, init_mib};
use libc::{c_int, c_ulong};

pub(crate) struct CpusWrapper {
    pub(crate) global_cpu_usage: f32,
    mib: [c_int; 2],
    previous: [c_ulong; libc::CPUSTATES as usize],
}
impl CpusWrapper {
    pub(crate) fn new() -> Self {
        let mut mib = [0; 2];
        unsafe {
            init_mib(b"kern.cp_time\0", &mut mib);
        }
        Self {
            global_cpu_usage: 0.0,
            mib,
            previous: [0; libc::CPUSTATES as usize],
        }
    }
    pub(crate) fn refresh(&mut self, sample_usage: bool) {
        if !sample_usage {
            return;
        }
        let mut current = [0; libc::CPUSTATES as usize];
        if unsafe { get_sys_value_array(&self.mib, &mut current) } {
            self.global_cpu_usage = compute_cpu_usage(&current, &self.previous);
            self.previous = current;
        }
    }
}

fn compute_cpu_usage(new_cp_time: &[c_ulong], old_cp_time: &[c_ulong]) -> f32 {
    let mut total_new: u64 = 0;
    let mut total_old: u64 = 0;
    let mut cp_diff: c_ulong = 0;

    for i in 0..(libc::CPUSTATES as usize) {
        // We obviously don't want to get the idle part of the CPU usage, otherwise
        // we would always be at 100%...
        if i != libc::CP_IDLE as usize {
            cp_diff = cp_diff.saturating_add(new_cp_time[i].saturating_sub(old_cp_time[i]));
        }
        total_new = total_new.saturating_add(new_cp_time[i] as _);
        total_old = total_old.saturating_add(old_cp_time[i] as _);
    }

    let total_diff = total_new.saturating_sub(total_old);
    if total_diff < 1 {
        0.
    } else {
        cp_diff as f32 / total_diff as f32 * 100.
    }
}
