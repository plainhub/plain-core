#[inline]
pub unsafe fn NtQuerySystemInformation(
    systeminformationclass: SYSTEM_INFORMATION_CLASS,
    systeminformation: *mut core::ffi::c_void,
    systeminformationlength: u32,
    returnlength: *mut u32,
) -> super::super::super::Win32::Foundation::NTSTATUS {
    windows_targets :: link ! ("ntdll.dll" "system" fn NtQuerySystemInformation (systeminformationclass : SYSTEM_INFORMATION_CLASS , systeminformation : * mut core :: ffi :: c_void , systeminformationlength : u32 , returnlength : * mut u32) -> super :: super :: super :: Win32 :: Foundation :: NTSTATUS);
    NtQuerySystemInformation(
        systeminformationclass,
        systeminformation,
        systeminformationlength,
        returnlength,
    )
}
pub const SystemProcessInformation: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(5i32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct SYSTEM_INFORMATION_CLASS(pub i32);
impl windows_core::TypeKind for SYSTEM_INFORMATION_CLASS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for SYSTEM_INFORMATION_CLASS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("SYSTEM_INFORMATION_CLASS")
            .field(&self.0)
            .finish()
    }
}
