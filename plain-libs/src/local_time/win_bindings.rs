#[link(name = "kernel32")]
unsafe extern "system" {
    pub(super) fn GetTimeZoneInformationForYear(
        year: u16,
        dynamic_info: *const core::ffi::c_void,
        info: *mut TimeZoneInformation,
    ) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct SystemTime {
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

#[repr(C)]
pub(super) struct TimeZoneInformation {
    pub bias: i32,
    standard_name: [u16; 32],
    pub standard_date: SystemTime,
    pub standard_bias: i32,
    daylight_name: [u16; 32],
    pub daylight_date: SystemTime,
    pub daylight_bias: i32,
}
