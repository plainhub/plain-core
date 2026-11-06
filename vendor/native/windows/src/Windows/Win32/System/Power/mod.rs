#[inline]
pub unsafe fn CallNtPowerInformation(
    informationlevel: POWER_INFORMATION_LEVEL,
    inputbuffer: Option<*const core::ffi::c_void>,
    inputbufferlength: u32,
    outputbuffer: Option<*mut core::ffi::c_void>,
    outputbufferlength: u32,
) -> super::super::Foundation::NTSTATUS {
    windows_targets :: link ! ("powrprof.dll" "system" fn CallNtPowerInformation (informationlevel : POWER_INFORMATION_LEVEL , inputbuffer : * const core :: ffi :: c_void , inputbufferlength : u32 , outputbuffer : * mut core :: ffi :: c_void , outputbufferlength : u32) -> super :: super :: Foundation :: NTSTATUS);
    CallNtPowerInformation(
        informationlevel,
        core::mem::transmute(inputbuffer.unwrap_or(std::ptr::null())),
        inputbufferlength,
        core::mem::transmute(outputbuffer.unwrap_or(std::ptr::null_mut())),
        outputbufferlength,
    )
}
pub const ProcessorInformation: POWER_INFORMATION_LEVEL = POWER_INFORMATION_LEVEL(11i32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct POWER_INFORMATION_LEVEL(pub i32);
impl windows_core::TypeKind for POWER_INFORMATION_LEVEL {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for POWER_INFORMATION_LEVEL {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("POWER_INFORMATION_LEVEL")
            .field(&self.0)
            .finish()
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PROCESSOR_POWER_INFORMATION {
    pub Number: u32,
    pub MaxMhz: u32,
    pub CurrentMhz: u32,
    pub MhzLimit: u32,
    pub MaxIdleState: u32,
    pub CurrentIdleState: u32,
}
impl windows_core::TypeKind for PROCESSOR_POWER_INFORMATION {
    type TypeKind = windows_core::CopyType;
}
impl Default for PROCESSOR_POWER_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
