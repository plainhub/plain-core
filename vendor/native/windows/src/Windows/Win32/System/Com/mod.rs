#[cfg(feature = "Win32_System_Com_StructuredStorage")]
pub mod StructuredStorage;
#[inline]
pub unsafe fn CoCreateInstance<P0, T>(
    rclsid: *const windows_core::GUID,
    punkouter: P0,
    dwclscontext: CLSCTX,
) -> windows_core::Result<T>
where
    P0: windows_core::Param<windows_core::IUnknown>,
    T: windows_core::Interface,
{
    windows_targets :: link ! ("ole32.dll" "system" fn CoCreateInstance (rclsid : * const windows_core :: GUID , punkouter : * mut core :: ffi :: c_void , dwclscontext : CLSCTX , riid : * const windows_core :: GUID , ppv : * mut * mut core :: ffi :: c_void) -> windows_core :: HRESULT);
    let mut result__ = core::ptr::null_mut();
    CoCreateInstance(
        rclsid,
        punkouter.param().abi(),
        dwclscontext,
        &T::IID,
        &mut result__,
    )
    .and_then(|| windows_core::Type::from_abi(result__))
}
#[inline]
pub unsafe fn CoInitializeEx(
    pvreserved: Option<*const core::ffi::c_void>,
    dwcoinit: COINIT,
) -> windows_core::HRESULT {
    windows_targets :: link ! ("ole32.dll" "system" fn CoInitializeEx (pvreserved : * const core :: ffi :: c_void , dwcoinit : u32) -> windows_core :: HRESULT);
    CoInitializeEx(
        core::mem::transmute(pvreserved.unwrap_or(std::ptr::null())),
        dwcoinit.0 as _,
    )
}
#[cfg(feature = "Win32_Security")]
#[inline]
pub unsafe fn CoInitializeSecurity<P0>(
    psecdesc: P0,
    cauthsvc: i32,
    asauthsvc: Option<*const SOLE_AUTHENTICATION_SERVICE>,
    preserved1: Option<*const core::ffi::c_void>,
    dwauthnlevel: RPC_C_AUTHN_LEVEL,
    dwimplevel: RPC_C_IMP_LEVEL,
    pauthlist: Option<*const core::ffi::c_void>,
    dwcapabilities: EOLE_AUTHENTICATION_CAPABILITIES,
    preserved3: Option<*const core::ffi::c_void>,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Security::PSECURITY_DESCRIPTOR>,
{
    windows_targets :: link ! ("ole32.dll" "system" fn CoInitializeSecurity (psecdesc : super :: super :: Security :: PSECURITY_DESCRIPTOR , cauthsvc : i32 , asauthsvc : * const SOLE_AUTHENTICATION_SERVICE , preserved1 : * const core :: ffi :: c_void , dwauthnlevel : RPC_C_AUTHN_LEVEL , dwimplevel : RPC_C_IMP_LEVEL , pauthlist : * const core :: ffi :: c_void , dwcapabilities : u32 , preserved3 : * const core :: ffi :: c_void) -> windows_core :: HRESULT);
    CoInitializeSecurity(
        psecdesc.param().abi(),
        cauthsvc,
        core::mem::transmute(asauthsvc.unwrap_or(std::ptr::null())),
        core::mem::transmute(preserved1.unwrap_or(std::ptr::null())),
        dwauthnlevel,
        dwimplevel,
        core::mem::transmute(pauthlist.unwrap_or(std::ptr::null())),
        dwcapabilities.0 as _,
        core::mem::transmute(preserved3.unwrap_or(std::ptr::null())),
    )
    .ok()
}
#[inline]
pub unsafe fn CoSetProxyBlanket<P0, P1>(
    pproxy: P0,
    dwauthnsvc: u32,
    dwauthzsvc: u32,
    pserverprincname: P1,
    dwauthnlevel: RPC_C_AUTHN_LEVEL,
    dwimplevel: RPC_C_IMP_LEVEL,
    pauthinfo: Option<*const core::ffi::c_void>,
    dwcapabilities: EOLE_AUTHENTICATION_CAPABILITIES,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<windows_core::IUnknown>,
    P1: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("ole32.dll" "system" fn CoSetProxyBlanket (pproxy : * mut core :: ffi :: c_void , dwauthnsvc : u32 , dwauthzsvc : u32 , pserverprincname : windows_core :: PCWSTR , dwauthnlevel : RPC_C_AUTHN_LEVEL , dwimplevel : RPC_C_IMP_LEVEL , pauthinfo : * const core :: ffi :: c_void , dwcapabilities : u32) -> windows_core :: HRESULT);
    CoSetProxyBlanket(
        pproxy.param().abi(),
        dwauthnsvc,
        dwauthzsvc,
        pserverprincname.param().abi(),
        dwauthnlevel,
        dwimplevel,
        core::mem::transmute(pauthinfo.unwrap_or(std::ptr::null())),
        dwcapabilities.0 as _,
    )
    .ok()
}
#[inline]
pub unsafe fn CoUninitialize() {
    windows_targets :: link ! ("ole32.dll" "system" fn CoUninitialize ());
    CoUninitialize()
}
windows_core::imp::define_interface!(
    IDispatch,
    IDispatch_Vtbl,
    0x00020400_0000_0000_c000_000000000046
);
impl core::ops::Deref for IDispatch {
    type Target = windows_core::IUnknown;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDispatch, windows_core::IUnknown);
impl IDispatch {}
#[repr(C)]
pub struct IDispatch_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetTypeInfoCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetTypeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetIDsOfNames: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const windows_core::PCWSTR,
        u32,
        u32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub Invoke: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        *const windows_core::GUID,
        u32,
        DISPATCH_FLAGS,
        *const DISPPARAMS,
        *mut core::mem::MaybeUninit<windows_core::VARIANT>,
        *mut EXCEPINFO,
        *mut u32,
    ) -> windows_core::HRESULT,
}
pub const CLSCTX_INPROC_SERVER: CLSCTX = CLSCTX(1u32);
pub const COINIT_MULTITHREADED: COINIT = COINIT(0i32);
pub const EOAC_NONE: EOLE_AUTHENTICATION_CAPABILITIES = EOLE_AUTHENTICATION_CAPABILITIES(0i32);
pub const RPC_C_AUTHN_LEVEL_CALL: RPC_C_AUTHN_LEVEL = RPC_C_AUTHN_LEVEL(3u32);
pub const RPC_C_AUTHN_LEVEL_DEFAULT: RPC_C_AUTHN_LEVEL = RPC_C_AUTHN_LEVEL(0u32);
pub const RPC_C_IMP_LEVEL_IMPERSONATE: RPC_C_IMP_LEVEL = RPC_C_IMP_LEVEL(3u32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct CLSCTX(pub u32);
impl windows_core::TypeKind for CLSCTX {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for CLSCTX {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("CLSCTX").field(&self.0).finish()
    }
}
impl CLSCTX {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for CLSCTX {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for CLSCTX {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for CLSCTX {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for CLSCTX {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for CLSCTX {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct COINIT(pub i32);
impl windows_core::TypeKind for COINIT {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for COINIT {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("COINIT").field(&self.0).finish()
    }
}
impl COINIT {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for COINIT {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for COINIT {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for COINIT {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for COINIT {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for COINIT {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct DISPATCH_FLAGS(pub u16);
impl windows_core::TypeKind for DISPATCH_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for DISPATCH_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DISPATCH_FLAGS").field(&self.0).finish()
    }
}
impl DISPATCH_FLAGS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for DISPATCH_FLAGS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for DISPATCH_FLAGS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for DISPATCH_FLAGS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for DISPATCH_FLAGS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for DISPATCH_FLAGS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct EOLE_AUTHENTICATION_CAPABILITIES(pub i32);
impl windows_core::TypeKind for EOLE_AUTHENTICATION_CAPABILITIES {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for EOLE_AUTHENTICATION_CAPABILITIES {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("EOLE_AUTHENTICATION_CAPABILITIES")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct RPC_C_AUTHN_LEVEL(pub u32);
impl windows_core::TypeKind for RPC_C_AUTHN_LEVEL {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for RPC_C_AUTHN_LEVEL {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("RPC_C_AUTHN_LEVEL").field(&self.0).finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct RPC_C_IMP_LEVEL(pub u32);
impl windows_core::TypeKind for RPC_C_IMP_LEVEL {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for RPC_C_IMP_LEVEL {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("RPC_C_IMP_LEVEL").field(&self.0).finish()
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DISPPARAMS {
    pub rgvarg: *mut windows_core::VARIANT,
    pub rgdispidNamedArgs: *mut i32,
    pub cArgs: u32,
    pub cNamedArgs: u32,
}
impl windows_core::TypeKind for DISPPARAMS {
    type TypeKind = windows_core::CopyType;
}
impl Default for DISPPARAMS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Debug)]
pub struct EXCEPINFO {
    pub wCode: u16,
    pub wReserved: u16,
    pub bstrSource: core::mem::ManuallyDrop<windows_core::BSTR>,
    pub bstrDescription: core::mem::ManuallyDrop<windows_core::BSTR>,
    pub bstrHelpFile: core::mem::ManuallyDrop<windows_core::BSTR>,
    pub dwHelpContext: u32,
    pub pvReserved: *mut core::ffi::c_void,
    pub pfnDeferredFillIn: LPEXCEPFINO_DEFERRED_FILLIN,
    pub scode: i32,
}
impl Clone for EXCEPINFO {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl windows_core::TypeKind for EXCEPINFO {
    type TypeKind = windows_core::CopyType;
}
impl Default for EXCEPINFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SOLE_AUTHENTICATION_SERVICE {
    pub dwAuthnSvc: u32,
    pub dwAuthzSvc: u32,
    pub pPrincipalName: windows_core::PWSTR,
    pub hr: windows_core::HRESULT,
}
impl windows_core::TypeKind for SOLE_AUTHENTICATION_SERVICE {
    type TypeKind = windows_core::CopyType;
}
impl Default for SOLE_AUTHENTICATION_SERVICE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type LPEXCEPFINO_DEFERRED_FILLIN =
    Option<unsafe extern "system" fn(pexcepinfo: *mut EXCEPINFO) -> windows_core::HRESULT>;
impl From<IDispatch> for windows_core::VARIANT {
    fn from(value: IDispatch) -> Self {
        unsafe {
            Self::from_raw(windows_core::imp::VARIANT {
                Anonymous: windows_core::imp::VARIANT_0 {
                    Anonymous: windows_core::imp::VARIANT_0_0 {
                        vt: 9,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: windows_core::imp::VARIANT_0_0_0 {
                            pdispVal: core::mem::transmute(value),
                        },
                    },
                },
            })
        }
    }
}
impl From<IDispatch> for windows_core::PROPVARIANT {
    fn from(value: IDispatch) -> Self {
        unsafe {
            Self::from_raw(windows_core::imp::PROPVARIANT {
                Anonymous: windows_core::imp::PROPVARIANT_0 {
                    Anonymous: windows_core::imp::PROPVARIANT_0_0 {
                        vt: 9,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: windows_core::imp::PROPVARIANT_0_0_0 {
                            pdispVal: core::mem::transmute(value),
                        },
                    },
                },
            })
        }
    }
}
impl TryFrom<&windows_core::VARIANT> for IDispatch {
    type Error = windows_core::Error;
    fn try_from(from: &windows_core::VARIANT) -> windows_core::Result<Self> {
        let from = from.as_raw();
        unsafe {
            if from.Anonymous.Anonymous.vt == 9
                && !from.Anonymous.Anonymous.Anonymous.pdispVal.is_null()
            {
                let dispatch: &IDispatch =
                    core::mem::transmute(&from.Anonymous.Anonymous.Anonymous.pdispVal);
                Ok(dispatch.clone())
            } else {
                Err(windows_core::Error::from_hresult(
                    windows_core::imp::TYPE_E_TYPEMISMATCH,
                ))
            }
        }
    }
}
impl TryFrom<&windows_core::PROPVARIANT> for IDispatch {
    type Error = windows_core::Error;
    fn try_from(from: &windows_core::PROPVARIANT) -> windows_core::Result<Self> {
        let from = from.as_raw();
        unsafe {
            if from.Anonymous.Anonymous.vt == 9
                && !from.Anonymous.Anonymous.Anonymous.pdispVal.is_null()
            {
                let dispatch: &IDispatch =
                    core::mem::transmute(&from.Anonymous.Anonymous.Anonymous.pdispVal);
                Ok(dispatch.clone())
            } else {
                Err(windows_core::Error::from_hresult(
                    windows_core::imp::TYPE_E_TYPEMISMATCH,
                ))
            }
        }
    }
}
#[cfg(any())]
core::include!("impl.rs");
