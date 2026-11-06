#[inline]
pub unsafe fn NtQueryInformationProcess<P0>(
    processhandle: P0,
    processinformationclass: PROCESSINFOCLASS,
    processinformation: *mut core::ffi::c_void,
    processinformationlength: u32,
    returnlength: *mut u32,
) -> super::super::super::Win32::Foundation::NTSTATUS
where
    P0: windows_core::Param<super::super::super::Win32::Foundation::HANDLE>,
{
    windows_targets :: link ! ("ntdll.dll" "system" fn NtQueryInformationProcess (processhandle : super :: super :: super :: Win32 :: Foundation :: HANDLE , processinformationclass : PROCESSINFOCLASS , processinformation : * mut core :: ffi :: c_void , processinformationlength : u32 , returnlength : * mut u32) -> super :: super :: super :: Win32 :: Foundation :: NTSTATUS);
    NtQueryInformationProcess(
        processhandle.param().abi(),
        processinformationclass,
        processinformation,
        processinformationlength,
        returnlength,
    )
}
pub const ProcessBasicInformation: PROCESSINFOCLASS = PROCESSINFOCLASS(0i32);
pub const ProcessCommandLineInformation: PROCESSINFOCLASS = PROCESSINFOCLASS(60i32);
pub const ProcessWow64Information: PROCESSINFOCLASS = PROCESSINFOCLASS(26i32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PROCESSINFOCLASS(pub i32);
impl windows_core::TypeKind for PROCESSINFOCLASS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PROCESSINFOCLASS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PROCESSINFOCLASS").field(&self.0).finish()
    }
}
