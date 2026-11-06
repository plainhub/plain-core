#[inline]
pub unsafe fn CloseHandle<P0>(hobject: P0) -> windows_core::Result<()>
where
    P0: windows_core::Param<HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn CloseHandle (hobject : HANDLE) -> BOOL);
    CloseHandle(hobject.param().abi()).ok()
}
#[inline]
pub unsafe fn FreeLibrary<P0>(hlibmodule: P0) -> windows_core::Result<()>
where
    P0: windows_core::Param<HMODULE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn FreeLibrary (hlibmodule : HMODULE) -> BOOL);
    FreeLibrary(hlibmodule.param().abi()).ok()
}
#[inline]
pub unsafe fn LocalFree<P0>(hmem: P0) -> HLOCAL
where
    P0: windows_core::Param<HLOCAL>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn LocalFree (hmem : HLOCAL) -> HLOCAL);
    LocalFree(hmem.param().abi())
}
#[inline]
pub unsafe fn SysAllocString<P0>(psz: P0) -> windows_core::BSTR
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("oleaut32.dll" "system" fn SysAllocString (psz : windows_core :: PCWSTR) -> windows_core :: BSTR);
    SysAllocString(psz.param().abi())
}
#[inline]
pub unsafe fn SysFreeString<P0>(bstrstring: P0)
where
    P0: windows_core::Param<windows_core::BSTR>,
{
    windows_targets :: link ! ("oleaut32.dll" "system" fn SysFreeString (bstrstring : core :: mem :: MaybeUninit < windows_core :: BSTR >));
    SysFreeString(bstrstring.param().abi())
}
pub const ERROR_ACCESS_DENIED: WIN32_ERROR = WIN32_ERROR(5u32);
pub const ERROR_BUFFER_OVERFLOW: WIN32_ERROR = WIN32_ERROR(111u32);
pub const ERROR_INSUFFICIENT_BUFFER: WIN32_ERROR = WIN32_ERROR(122u32);
pub const ERROR_INVALID_LEVEL: WIN32_ERROR = WIN32_ERROR(124u32);
pub const ERROR_MORE_DATA: WIN32_ERROR = WIN32_ERROR(234u32);
pub const ERROR_NO_MORE_FILES: WIN32_ERROR = WIN32_ERROR(18u32);
pub const ERROR_SUCCESS: WIN32_ERROR = WIN32_ERROR(0u32);
pub const FALSE: BOOL = BOOL(0i32);
pub const MAX_PATH: u32 = 260u32;
pub const STATUS_BUFFER_OVERFLOW: NTSTATUS = NTSTATUS(0x80000005_u32 as _);
pub const STATUS_BUFFER_TOO_SMALL: NTSTATUS = NTSTATUS(0xC0000023_u32 as _);
pub const STATUS_INFO_LENGTH_MISMATCH: NTSTATUS = NTSTATUS(0xC0000004_u32 as _);
pub const STILL_ACTIVE: NTSTATUS = NTSTATUS(0x103_u32 as _);
pub const TYPE_E_TYPEMISMATCH: windows_core::HRESULT = windows_core::HRESULT(0x80028CA0_u32 as _);
pub const VARIANT_FALSE: VARIANT_BOOL = VARIANT_BOOL(0i16);
pub const VARIANT_TRUE: VARIANT_BOOL = VARIANT_BOOL(-1i16);
#[must_use]
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct WIN32_ERROR(pub u32);
impl windows_core::TypeKind for WIN32_ERROR {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for WIN32_ERROR {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("WIN32_ERROR").field(&self.0).finish()
    }
}
#[must_use]
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BOOL(pub i32);
impl Default for BOOL {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for BOOL {
    type TypeKind = windows_core::CopyType;
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BOOLEAN(pub u8);
impl Default for BOOLEAN {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for BOOLEAN {
    type TypeKind = windows_core::CopyType;
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}
impl windows_core::TypeKind for FILETIME {
    type TypeKind = windows_core::CopyType;
}
impl Default for FILETIME {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HANDLE(pub isize);
impl HANDLE {
    pub fn is_invalid(&self) -> bool {
        self.0 == -1 || self.0 == 0
    }
}
impl windows_core::Free for HANDLE {
    unsafe fn free(&mut self) {
        if !self.is_invalid() {
            _ = CloseHandle(*self);
        }
    }
}
impl Default for HANDLE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for HANDLE {
    type TypeKind = windows_core::CopyType;
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HINSTANCE(pub isize);
impl HINSTANCE {
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }
}
impl windows_core::Free for HINSTANCE {
    unsafe fn free(&mut self) {
        if !self.is_invalid() {
            _ = FreeLibrary(*self);
        }
    }
}
impl Default for HINSTANCE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for HINSTANCE {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::imp::CanInto<HMODULE> for HINSTANCE {}
impl From<HINSTANCE> for HMODULE {
    fn from(value: HINSTANCE) -> Self {
        Self(value.0)
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HLOCAL(pub *mut core::ffi::c_void);
impl HLOCAL {
    pub fn is_invalid(&self) -> bool {
        self.0.is_null()
    }
}
impl windows_core::Free for HLOCAL {
    unsafe fn free(&mut self) {
        if !self.is_invalid() {
            _ = LocalFree(*self);
        }
    }
}
impl Default for HLOCAL {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for HLOCAL {
    type TypeKind = windows_core::CopyType;
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HMODULE(pub isize);
impl HMODULE {
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }
}
impl windows_core::Free for HMODULE {
    unsafe fn free(&mut self) {
        if !self.is_invalid() {
            _ = FreeLibrary(*self);
        }
    }
}
impl Default for HMODULE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for HMODULE {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::imp::CanInto<HINSTANCE> for HMODULE {}
impl From<HMODULE> for HINSTANCE {
    fn from(value: HMODULE) -> Self {
        Self(value.0)
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HWND(pub isize);
impl Default for HWND {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for HWND {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::imp::CanInto<HANDLE> for HWND {}
impl From<HWND> for HANDLE {
    fn from(value: HWND) -> Self {
        Self(value.0)
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LUID {
    pub LowPart: u32,
    pub HighPart: i32,
}
impl windows_core::TypeKind for LUID {
    type TypeKind = windows_core::CopyType;
}
impl Default for LUID {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[must_use]
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NTSTATUS(pub i32);
impl Default for NTSTATUS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for NTSTATUS {
    type TypeKind = windows_core::CopyType;
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PSID(pub *mut core::ffi::c_void);
impl PSID {
    pub fn is_invalid(&self) -> bool {
        self.0.is_null()
    }
}
impl Default for PSID {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for PSID {
    type TypeKind = windows_core::CopyType;
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: windows_core::PWSTR,
}
impl windows_core::TypeKind for UNICODE_STRING {
    type TypeKind = windows_core::CopyType;
}
impl Default for UNICODE_STRING {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VARIANT_BOOL(pub i16);
impl Default for VARIANT_BOOL {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
impl windows_core::TypeKind for VARIANT_BOOL {
    type TypeKind = windows_core::CopyType;
}
impl BOOL {
    #[inline]
    pub fn as_bool(self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub fn ok(self) -> windows_core::Result<()> {
        if self.as_bool() {
            Ok(())
        } else {
            Err(windows_core::Error::from_win32())
        }
    }
    #[inline]
    #[track_caller]
    pub fn unwrap(self) {
        self.ok().unwrap();
    }
    #[inline]
    #[track_caller]
    pub fn expect(self, msg: &str) {
        self.ok().expect(msg);
    }
}
impl From<BOOL> for bool {
    fn from(value: BOOL) -> Self {
        value.as_bool()
    }
}
impl From<&BOOL> for bool {
    fn from(value: &BOOL) -> Self {
        value.as_bool()
    }
}
impl From<bool> for BOOL {
    fn from(value: bool) -> Self {
        if value {
            Self(1)
        } else {
            Self(0)
        }
    }
}
impl From<&bool> for BOOL {
    fn from(value: &bool) -> Self {
        (*value).into()
    }
}
impl PartialEq<bool> for BOOL {
    fn eq(&self, other: &bool) -> bool {
        self.as_bool() == *other
    }
}
impl PartialEq<BOOL> for bool {
    fn eq(&self, other: &BOOL) -> bool {
        *self == other.as_bool()
    }
}
impl core::ops::Not for BOOL {
    type Output = Self;
    fn not(self) -> Self::Output {
        if self.as_bool() {
            Self(0)
        } else {
            Self(1)
        }
    }
}
impl windows_core::Param<BOOL> for bool {
    unsafe fn param(self) -> windows_core::ParamValue<BOOL> {
        windows_core::ParamValue::Owned(self.into())
    }
}
impl BOOLEAN {
    #[inline]
    pub fn as_bool(self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub fn ok(self) -> windows_core::Result<()> {
        if self.as_bool() {
            Ok(())
        } else {
            Err(windows_core::Error::from_win32())
        }
    }
    #[inline]
    #[track_caller]
    pub fn unwrap(self) {
        self.ok().unwrap();
    }
    #[inline]
    #[track_caller]
    pub fn expect(self, msg: &str) {
        self.ok().expect(msg);
    }
}
impl From<BOOLEAN> for bool {
    fn from(value: BOOLEAN) -> Self {
        value.as_bool()
    }
}
impl From<&BOOLEAN> for bool {
    fn from(value: &BOOLEAN) -> Self {
        value.as_bool()
    }
}
impl From<bool> for BOOLEAN {
    fn from(value: bool) -> Self {
        if value {
            Self(1)
        } else {
            Self(0)
        }
    }
}
impl From<&bool> for BOOLEAN {
    fn from(value: &bool) -> Self {
        (*value).into()
    }
}
impl PartialEq<bool> for BOOLEAN {
    fn eq(&self, other: &bool) -> bool {
        self.as_bool() == *other
    }
}
impl PartialEq<BOOLEAN> for bool {
    fn eq(&self, other: &BOOLEAN) -> bool {
        *self == other.as_bool()
    }
}
impl core::ops::Not for BOOLEAN {
    type Output = Self;
    fn not(self) -> Self::Output {
        if self.as_bool() {
            Self(0)
        } else {
            Self(1)
        }
    }
}
impl windows_core::Param<BOOLEAN> for bool {
    unsafe fn param(self) -> windows_core::ParamValue<BOOLEAN> {
        windows_core::ParamValue::Owned(self.into())
    }
}
impl NTSTATUS {
    #[inline]
    pub const fn is_ok(self) -> bool {
        self.0 >= 0
    }
    #[inline]
    pub const fn is_err(self) -> bool {
        !self.is_ok()
    }
    #[inline]
    pub const fn to_hresult(self) -> windows_core::HRESULT {
        windows_core::HRESULT::from_nt(self.0)
    }
    #[inline]
    pub fn ok(self) -> windows_core::Result<()> {
        if self.is_ok() {
            Ok(())
        } else {
            Err(self.to_hresult().into())
        }
    }
}
impl From<NTSTATUS> for windows_core::HRESULT {
    fn from(value: NTSTATUS) -> Self {
        value.to_hresult()
    }
}
impl From<NTSTATUS> for windows_core::Error {
    fn from(value: NTSTATUS) -> Self {
        value.to_hresult().into()
    }
}
impl VARIANT_BOOL {
    #[inline]
    pub fn as_bool(self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub fn ok(self) -> windows_core::Result<()> {
        if self.as_bool() {
            Ok(())
        } else {
            Err(windows_core::Error::from_win32())
        }
    }
    #[inline]
    #[track_caller]
    pub fn unwrap(self) {
        self.ok().unwrap();
    }
    #[inline]
    #[track_caller]
    pub fn expect(self, msg: &str) {
        self.ok().expect(msg);
    }
}
impl From<VARIANT_BOOL> for bool {
    fn from(value: VARIANT_BOOL) -> Self {
        value.as_bool()
    }
}
impl From<&VARIANT_BOOL> for bool {
    fn from(value: &VARIANT_BOOL) -> Self {
        value.as_bool()
    }
}
impl From<bool> for VARIANT_BOOL {
    fn from(value: bool) -> Self {
        if value {
            VARIANT_TRUE
        } else {
            VARIANT_FALSE
        }
    }
}
impl From<&bool> for VARIANT_BOOL {
    fn from(value: &bool) -> Self {
        (*value).into()
    }
}
impl PartialEq<bool> for VARIANT_BOOL {
    fn eq(&self, other: &bool) -> bool {
        self.as_bool() == *other
    }
}
impl PartialEq<VARIANT_BOOL> for bool {
    fn eq(&self, other: &VARIANT_BOOL) -> bool {
        *self == other.as_bool()
    }
}
impl core::ops::Not for VARIANT_BOOL {
    type Output = Self;
    fn not(self) -> Self::Output {
        if self.as_bool() {
            VARIANT_FALSE
        } else {
            VARIANT_TRUE
        }
    }
}
impl WIN32_ERROR {
    #[inline]
    pub const fn is_ok(self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub const fn is_err(self) -> bool {
        !self.is_ok()
    }
    #[inline]
    pub const fn to_hresult(self) -> windows_core::HRESULT {
        windows_core::HRESULT::from_win32(self.0)
    }
    #[inline]
    pub fn ok(self) -> windows_core::Result<()> {
        if self.is_ok() {
            Ok(())
        } else {
            Err(self.to_hresult().into())
        }
    }
}
impl From<WIN32_ERROR> for windows_core::HRESULT {
    fn from(value: WIN32_ERROR) -> Self {
        value.to_hresult()
    }
}
impl From<WIN32_ERROR> for windows_core::Error {
    fn from(value: WIN32_ERROR) -> Self {
        value.to_hresult().into()
    }
}
