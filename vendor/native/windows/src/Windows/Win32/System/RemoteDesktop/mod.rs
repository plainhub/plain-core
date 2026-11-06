#[inline]
pub unsafe fn ProcessIdToSessionId(
    dwprocessid: u32,
    psessionid: *mut u32,
) -> windows_core::Result<()> {
    windows_targets :: link ! ("kernel32.dll" "system" fn ProcessIdToSessionId (dwprocessid : u32 , psessionid : * mut u32) -> super :: super :: Foundation :: BOOL);
    ProcessIdToSessionId(dwprocessid, psessionid).ok()
}
#[cfg(any())]
core::include!("impl.rs");
