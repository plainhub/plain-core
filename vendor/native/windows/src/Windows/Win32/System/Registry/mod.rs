#[inline]
pub unsafe fn RegCloseKey<P0>(hkey: P0) -> super::super::Foundation::WIN32_ERROR
where
    P0: windows_core::Param<HKEY>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn RegCloseKey (hkey : HKEY) -> super :: super :: Foundation :: WIN32_ERROR);
    RegCloseKey(hkey.param().abi())
}
#[inline]
pub unsafe fn RegOpenKeyExW<P0, P1>(
    hkey: P0,
    lpsubkey: P1,
    uloptions: u32,
    samdesired: REG_SAM_FLAGS,
    phkresult: *mut HKEY,
) -> super::super::Foundation::WIN32_ERROR
where
    P0: windows_core::Param<HKEY>,
    P1: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn RegOpenKeyExW (hkey : HKEY , lpsubkey : windows_core :: PCWSTR , uloptions : u32 , samdesired : REG_SAM_FLAGS , phkresult : * mut HKEY) -> super :: super :: Foundation :: WIN32_ERROR);
    RegOpenKeyExW(
        hkey.param().abi(),
        lpsubkey.param().abi(),
        uloptions,
        samdesired,
        phkresult,
    )
}
#[inline]
pub unsafe fn RegQueryValueExW<P0, P1>(
    hkey: P0,
    lpvaluename: P1,
    lpreserved: Option<*const u32>,
    lptype: Option<*mut REG_VALUE_TYPE>,
    lpdata: Option<*mut u8>,
    lpcbdata: Option<*mut u32>,
) -> super::super::Foundation::WIN32_ERROR
where
    P0: windows_core::Param<HKEY>,
    P1: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn RegQueryValueExW (hkey : HKEY , lpvaluename : windows_core :: PCWSTR , lpreserved : * const u32 , lptype : * mut REG_VALUE_TYPE , lpdata : * mut u8 , lpcbdata : * mut u32) -> super :: super :: Foundation :: WIN32_ERROR);
    RegQueryValueExW(
        hkey.param().abi(),
        lpvaluename.param().abi(),
        core::mem::transmute(lpreserved.unwrap_or(std::ptr::null())),
        core::mem::transmute(lptype.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpdata.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpcbdata.unwrap_or(std::ptr::null_mut())),
    )
}
pub const HKEY_LOCAL_MACHINE: HKEY = HKEY(-2147483646i32 as _);
pub const KEY_READ: REG_SAM_FLAGS = REG_SAM_FLAGS(131097u32);
pub const REG_NONE: REG_VALUE_TYPE = REG_VALUE_TYPE(0u32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct REG_SAM_FLAGS(pub u32);
impl windows_core::TypeKind for REG_SAM_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for REG_SAM_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("REG_SAM_FLAGS").field(&self.0).finish()
    }
}
impl REG_SAM_FLAGS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for REG_SAM_FLAGS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for REG_SAM_FLAGS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for REG_SAM_FLAGS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for REG_SAM_FLAGS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for REG_SAM_FLAGS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct REG_VALUE_TYPE(pub u32);
impl windows_core::TypeKind for REG_VALUE_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for REG_VALUE_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("REG_VALUE_TYPE").field(&self.0).finish()
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HKEY(pub isize);
impl HKEY {
    pub fn is_invalid(&self) -> bool {
        self.0 == -1 || self.0 == 0
    }
}
impl windows_core::Free for HKEY {
    unsafe fn free(&mut self) {
        if !self.is_invalid() {
            _ = RegCloseKey(*self);
        }
    }
}
impl Default for HKEY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for HKEY {
    type TypeKind = windows_core::CopyType;
}
