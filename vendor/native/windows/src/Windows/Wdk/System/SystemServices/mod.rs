#[cfg(feature = "Win32_System_SystemInformation")]
#[inline]
pub unsafe fn RtlGetVersion(
    lpversioninformation : * mut super :: super :: super :: Win32 :: System :: SystemInformation :: OSVERSIONINFOW,
) -> super::super::super::Win32::Foundation::NTSTATUS {
    windows_targets :: link ! ("ntdll.dll" "system" fn RtlGetVersion (lpversioninformation : * mut super :: super :: super :: Win32 :: System :: SystemInformation :: OSVERSIONINFOW) -> super :: super :: super :: Win32 :: Foundation :: NTSTATUS);
    RtlGetVersion(lpversioninformation)
}
