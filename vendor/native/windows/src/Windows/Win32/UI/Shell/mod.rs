#[inline]
pub unsafe fn CommandLineToArgvW<P0>(lpcmdline: P0, pnumargs: *mut i32) -> *mut windows_core::PWSTR
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("shell32.dll" "system" fn CommandLineToArgvW (lpcmdline : windows_core :: PCWSTR , pnumargs : * mut i32) -> * mut windows_core :: PWSTR);
    CommandLineToArgvW(lpcmdline.param().abi(), pnumargs)
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct OS(pub u32);
impl windows_core::TypeKind for OS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for OS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("OS").field(&self.0).finish()
    }
}
pub const Shell: windows_core::GUID =
    windows_core::GUID::from_u128(0x13709620_c279_11ce_a49e_444553540000);
#[cfg(any())]
core::include!("impl.rs");
