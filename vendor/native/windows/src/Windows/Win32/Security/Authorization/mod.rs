#[inline]
pub unsafe fn ConvertSidToStringSidW<P0>(
    sid: P0,
    stringsid: *mut windows_core::PWSTR,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::PSID>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn ConvertSidToStringSidW (sid : super :: super :: Foundation :: PSID , stringsid : * mut windows_core :: PWSTR) -> super :: super :: Foundation :: BOOL);
    ConvertSidToStringSidW(sid.param().abi(), stringsid).ok()
}
#[inline]
pub unsafe fn ConvertStringSidToSidW<P0>(
    stringsid: P0,
    sid: *mut super::super::Foundation::PSID,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn ConvertStringSidToSidW (stringsid : windows_core :: PCWSTR , sid : * mut super :: super :: Foundation :: PSID) -> super :: super :: Foundation :: BOOL);
    ConvertStringSidToSidW(stringsid.param().abi(), sid).ok()
}
#[cfg(any())]
core::include!("impl.rs");
