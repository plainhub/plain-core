#[cfg(feature = "Win32_Security_Authorization")]
pub mod Authorization;
#[inline]
pub unsafe fn CopySid<P0>(
    ndestinationsidlength: u32,
    pdestinationsid: super::Foundation::PSID,
    psourcesid: P0,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::Foundation::PSID>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn CopySid (ndestinationsidlength : u32 , pdestinationsid : super :: Foundation :: PSID , psourcesid : super :: Foundation :: PSID) -> super :: Foundation :: BOOL);
    CopySid(
        ndestinationsidlength,
        pdestinationsid,
        psourcesid.param().abi(),
    )
    .ok()
}
#[inline]
pub unsafe fn GetLengthSid(psid: super::Foundation::PSID) -> u32 {
    windows_targets :: link ! ("advapi32.dll" "system" fn GetLengthSid (psid : super :: Foundation :: PSID) -> u32);
    GetLengthSid(psid)
}
#[inline]
pub unsafe fn GetTokenInformation<P0>(
    tokenhandle: P0,
    tokeninformationclass: TOKEN_INFORMATION_CLASS,
    tokeninformation: Option<*mut core::ffi::c_void>,
    tokeninformationlength: u32,
    returnlength: *mut u32,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn GetTokenInformation (tokenhandle : super :: Foundation :: HANDLE , tokeninformationclass : TOKEN_INFORMATION_CLASS , tokeninformation : * mut core :: ffi :: c_void , tokeninformationlength : u32 , returnlength : * mut u32) -> super :: Foundation :: BOOL);
    GetTokenInformation(
        tokenhandle.param().abi(),
        tokeninformationclass,
        core::mem::transmute(tokeninformation.unwrap_or(std::ptr::null_mut())),
        tokeninformationlength,
        returnlength,
    )
    .ok()
}
#[inline]
pub unsafe fn IsValidSid<P0>(psid: P0) -> super::Foundation::BOOL
where
    P0: windows_core::Param<super::Foundation::PSID>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn IsValidSid (psid : super :: Foundation :: PSID) -> super :: Foundation :: BOOL);
    IsValidSid(psid.param().abi())
}
#[inline]
pub unsafe fn LookupAccountSidW<P0, P1>(
    lpsystemname: P0,
    sid: P1,
    name: windows_core::PWSTR,
    cchname: *mut u32,
    referenceddomainname: windows_core::PWSTR,
    cchreferenceddomainname: *mut u32,
    peuse: *mut SID_NAME_USE,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
    P1: windows_core::Param<super::Foundation::PSID>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn LookupAccountSidW (lpsystemname : windows_core :: PCWSTR , sid : super :: Foundation :: PSID , name : windows_core :: PWSTR , cchname : * mut u32 , referenceddomainname : windows_core :: PWSTR , cchreferenceddomainname : * mut u32 , peuse : * mut SID_NAME_USE) -> super :: Foundation :: BOOL);
    LookupAccountSidW(
        lpsystemname.param().abi(),
        sid.param().abi(),
        core::mem::transmute(name),
        cchname,
        core::mem::transmute(referenceddomainname),
        cchreferenceddomainname,
        peuse,
    )
    .ok()
}
pub const SidTypeUnknown: SID_NAME_USE = SID_NAME_USE(8i32);
pub const TOKEN_QUERY: TOKEN_ACCESS_MASK = TOKEN_ACCESS_MASK(8u32);
pub const TokenUser: TOKEN_INFORMATION_CLASS = TOKEN_INFORMATION_CLASS(1i32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct SID_NAME_USE(pub i32);
impl windows_core::TypeKind for SID_NAME_USE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for SID_NAME_USE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("SID_NAME_USE").field(&self.0).finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct TOKEN_ACCESS_MASK(pub u32);
impl windows_core::TypeKind for TOKEN_ACCESS_MASK {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for TOKEN_ACCESS_MASK {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("TOKEN_ACCESS_MASK").field(&self.0).finish()
    }
}
impl TOKEN_ACCESS_MASK {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for TOKEN_ACCESS_MASK {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for TOKEN_ACCESS_MASK {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for TOKEN_ACCESS_MASK {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for TOKEN_ACCESS_MASK {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for TOKEN_ACCESS_MASK {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct TOKEN_INFORMATION_CLASS(pub i32);
impl windows_core::TypeKind for TOKEN_INFORMATION_CLASS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for TOKEN_INFORMATION_CLASS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("TOKEN_INFORMATION_CLASS")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PSECURITY_DESCRIPTOR(pub *mut core::ffi::c_void);
impl PSECURITY_DESCRIPTOR {
    pub fn is_invalid(&self) -> bool {
        self.0.is_null()
    }
}
impl Default for PSECURITY_DESCRIPTOR {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for PSECURITY_DESCRIPTOR {
    type TypeKind = windows_core::CopyType;
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SECURITY_ATTRIBUTES {
    pub nLength: u32,
    pub lpSecurityDescriptor: *mut core::ffi::c_void,
    pub bInheritHandle: super::Foundation::BOOL,
}
impl windows_core::TypeKind for SECURITY_ATTRIBUTES {
    type TypeKind = windows_core::CopyType;
}
impl Default for SECURITY_ATTRIBUTES {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SID_AND_ATTRIBUTES {
    pub Sid: super::Foundation::PSID,
    pub Attributes: u32,
}
impl windows_core::TypeKind for SID_AND_ATTRIBUTES {
    type TypeKind = windows_core::CopyType;
}
impl Default for SID_AND_ATTRIBUTES {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TOKEN_USER {
    pub User: SID_AND_ATTRIBUTES,
}
impl windows_core::TypeKind for TOKEN_USER {
    type TypeKind = windows_core::CopyType;
}
impl Default for TOKEN_USER {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
