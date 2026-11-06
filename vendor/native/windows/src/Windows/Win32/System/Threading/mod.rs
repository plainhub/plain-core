#[cfg(feature = "Win32_Security")]
#[inline]
pub unsafe fn CreateEventA<P0, P1, P2>(
    lpeventattributes: Option<*const super::super::Security::SECURITY_ATTRIBUTES>,
    bmanualreset: P0,
    binitialstate: P1,
    lpname: P2,
) -> windows_core::Result<super::super::Foundation::HANDLE>
where
    P0: windows_core::Param<super::super::Foundation::BOOL>,
    P1: windows_core::Param<super::super::Foundation::BOOL>,
    P2: windows_core::Param<windows_core::PCSTR>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn CreateEventA (lpeventattributes : * const super :: super :: Security :: SECURITY_ATTRIBUTES , bmanualreset : super :: super :: Foundation :: BOOL , binitialstate : super :: super :: Foundation :: BOOL , lpname : windows_core :: PCSTR) -> super :: super :: Foundation :: HANDLE);
    let result__ = CreateEventA(
        core::mem::transmute(lpeventattributes.unwrap_or(std::ptr::null())),
        bmanualreset.param().abi(),
        binitialstate.param().abi(),
        lpname.param().abi(),
    );
    (!result__.is_invalid())
        .then(|| result__)
        .ok_or_else(windows_core::Error::from_win32)
}
#[inline]
pub unsafe fn GetCurrentProcessId() -> u32 {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetCurrentProcessId () -> u32);
    GetCurrentProcessId()
}
#[inline]
pub unsafe fn GetExitCodeProcess<P0>(hprocess: P0, lpexitcode: *mut u32) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetExitCodeProcess (hprocess : super :: super :: Foundation :: HANDLE , lpexitcode : * mut u32) -> super :: super :: Foundation :: BOOL);
    GetExitCodeProcess(hprocess.param().abi(), lpexitcode).ok()
}
#[inline]
pub unsafe fn GetProcessIoCounters<P0>(
    hprocess: P0,
    lpiocounters: *mut IO_COUNTERS,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetProcessIoCounters (hprocess : super :: super :: Foundation :: HANDLE , lpiocounters : * mut IO_COUNTERS) -> super :: super :: Foundation :: BOOL);
    GetProcessIoCounters(hprocess.param().abi(), lpiocounters).ok()
}
#[inline]
pub unsafe fn GetProcessTimes<P0>(
    hprocess: P0,
    lpcreationtime: *mut super::super::Foundation::FILETIME,
    lpexittime: *mut super::super::Foundation::FILETIME,
    lpkerneltime: *mut super::super::Foundation::FILETIME,
    lpusertime: *mut super::super::Foundation::FILETIME,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetProcessTimes (hprocess : super :: super :: Foundation :: HANDLE , lpcreationtime : * mut super :: super :: Foundation :: FILETIME , lpexittime : * mut super :: super :: Foundation :: FILETIME , lpkerneltime : * mut super :: super :: Foundation :: FILETIME , lpusertime : * mut super :: super :: Foundation :: FILETIME) -> super :: super :: Foundation :: BOOL);
    GetProcessTimes(
        hprocess.param().abi(),
        lpcreationtime,
        lpexittime,
        lpkerneltime,
        lpusertime,
    )
    .ok()
}
#[inline]
pub unsafe fn GetSystemTimes(
    lpidletime: Option<*mut super::super::Foundation::FILETIME>,
    lpkerneltime: Option<*mut super::super::Foundation::FILETIME>,
    lpusertime: Option<*mut super::super::Foundation::FILETIME>,
) -> windows_core::Result<()> {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetSystemTimes (lpidletime : * mut super :: super :: Foundation :: FILETIME , lpkerneltime : * mut super :: super :: Foundation :: FILETIME , lpusertime : * mut super :: super :: Foundation :: FILETIME) -> super :: super :: Foundation :: BOOL);
    GetSystemTimes(
        core::mem::transmute(lpidletime.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpkerneltime.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpusertime.unwrap_or(std::ptr::null_mut())),
    )
    .ok()
}
#[inline]
pub unsafe fn OpenProcess<P0>(
    dwdesiredaccess: PROCESS_ACCESS_RIGHTS,
    binherithandle: P0,
    dwprocessid: u32,
) -> windows_core::Result<super::super::Foundation::HANDLE>
where
    P0: windows_core::Param<super::super::Foundation::BOOL>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn OpenProcess (dwdesiredaccess : PROCESS_ACCESS_RIGHTS , binherithandle : super :: super :: Foundation :: BOOL , dwprocessid : u32) -> super :: super :: Foundation :: HANDLE);
    let result__ = OpenProcess(dwdesiredaccess, binherithandle.param().abi(), dwprocessid);
    (!result__.is_invalid())
        .then(|| result__)
        .ok_or_else(windows_core::Error::from_win32)
}
#[cfg(feature = "Win32_Security")]
#[inline]
pub unsafe fn OpenProcessToken<P0>(
    processhandle: P0,
    desiredaccess: super::super::Security::TOKEN_ACCESS_MASK,
    tokenhandle: *mut super::super::Foundation::HANDLE,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("advapi32.dll" "system" fn OpenProcessToken (processhandle : super :: super :: Foundation :: HANDLE , desiredaccess : super :: super :: Security :: TOKEN_ACCESS_MASK , tokenhandle : * mut super :: super :: Foundation :: HANDLE) -> super :: super :: Foundation :: BOOL);
    OpenProcessToken(processhandle.param().abi(), desiredaccess, tokenhandle).ok()
}
#[inline]
pub unsafe fn RegisterWaitForSingleObject<P0>(
    phnewwaitobject: *mut super::super::Foundation::HANDLE,
    hobject: P0,
    callback: WAITORTIMERCALLBACK,
    context: Option<*const core::ffi::c_void>,
    dwmilliseconds: u32,
    dwflags: WORKER_THREAD_FLAGS,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn RegisterWaitForSingleObject (phnewwaitobject : * mut super :: super :: Foundation :: HANDLE , hobject : super :: super :: Foundation :: HANDLE , callback : WAITORTIMERCALLBACK , context : * const core :: ffi :: c_void , dwmilliseconds : u32 , dwflags : WORKER_THREAD_FLAGS) -> super :: super :: Foundation :: BOOL);
    RegisterWaitForSingleObject(
        phnewwaitobject,
        hobject.param().abi(),
        callback,
        core::mem::transmute(context.unwrap_or(std::ptr::null())),
        dwmilliseconds,
        dwflags,
    )
    .ok()
}
#[inline]
pub unsafe fn Sleep(dwmilliseconds: u32) {
    windows_targets :: link ! ("kernel32.dll" "system" fn Sleep (dwmilliseconds : u32));
    Sleep(dwmilliseconds)
}
pub const CREATE_NO_WINDOW: PROCESS_CREATION_FLAGS = PROCESS_CREATION_FLAGS(134217728u32);
pub const INFINITE: u32 = 4294967295u32;
pub const PROCESS_QUERY_INFORMATION: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(1024u32);
pub const PROCESS_QUERY_LIMITED_INFORMATION: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(4096u32);
pub const PROCESS_VM_READ: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(16u32);
pub const WT_EXECUTEDEFAULT: WORKER_THREAD_FLAGS = WORKER_THREAD_FLAGS(0u32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PROCESS_ACCESS_RIGHTS(pub u32);
impl windows_core::TypeKind for PROCESS_ACCESS_RIGHTS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PROCESS_ACCESS_RIGHTS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PROCESS_ACCESS_RIGHTS")
            .field(&self.0)
            .finish()
    }
}
impl PROCESS_ACCESS_RIGHTS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for PROCESS_ACCESS_RIGHTS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for PROCESS_ACCESS_RIGHTS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for PROCESS_ACCESS_RIGHTS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for PROCESS_ACCESS_RIGHTS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for PROCESS_ACCESS_RIGHTS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PROCESS_CREATION_FLAGS(pub u32);
impl windows_core::TypeKind for PROCESS_CREATION_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PROCESS_CREATION_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PROCESS_CREATION_FLAGS")
            .field(&self.0)
            .finish()
    }
}
impl PROCESS_CREATION_FLAGS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for PROCESS_CREATION_FLAGS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for PROCESS_CREATION_FLAGS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for PROCESS_CREATION_FLAGS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for PROCESS_CREATION_FLAGS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for PROCESS_CREATION_FLAGS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct WORKER_THREAD_FLAGS(pub u32);
impl windows_core::TypeKind for WORKER_THREAD_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for WORKER_THREAD_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("WORKER_THREAD_FLAGS").field(&self.0).finish()
    }
}
impl WORKER_THREAD_FLAGS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for WORKER_THREAD_FLAGS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for WORKER_THREAD_FLAGS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for WORKER_THREAD_FLAGS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for WORKER_THREAD_FLAGS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for WORKER_THREAD_FLAGS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IO_COUNTERS {
    pub ReadOperationCount: u64,
    pub WriteOperationCount: u64,
    pub OtherOperationCount: u64,
    pub ReadTransferCount: u64,
    pub WriteTransferCount: u64,
    pub OtherTransferCount: u64,
}
impl windows_core::TypeKind for IO_COUNTERS {
    type TypeKind = windows_core::CopyType;
}
impl Default for IO_COUNTERS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[cfg(feature = "Win32_System_Kernel")]
#[derive(Clone, Copy, Debug)]
pub struct PEB {
    pub Reserved1: [u8; 2],
    pub BeingDebugged: u8,
    pub Reserved2: [u8; 1],
    pub Reserved3: [*mut core::ffi::c_void; 2],
    pub Ldr: *mut PEB_LDR_DATA,
    pub ProcessParameters: *mut RTL_USER_PROCESS_PARAMETERS,
    pub Reserved4: [*mut core::ffi::c_void; 3],
    pub AtlThunkSListPtr: *mut core::ffi::c_void,
    pub Reserved5: *mut core::ffi::c_void,
    pub Reserved6: u32,
    pub Reserved7: *mut core::ffi::c_void,
    pub Reserved8: u32,
    pub AtlThunkSListPtr32: u32,
    pub Reserved9: [*mut core::ffi::c_void; 45],
    pub Reserved10: [u8; 96],
    pub PostProcessInitRoutine: PPS_POST_PROCESS_INIT_ROUTINE,
    pub Reserved11: [u8; 128],
    pub Reserved12: [*mut core::ffi::c_void; 1],
    pub SessionId: u32,
}
#[cfg(feature = "Win32_System_Kernel")]
impl windows_core::TypeKind for PEB {
    type TypeKind = windows_core::CopyType;
}
#[cfg(feature = "Win32_System_Kernel")]
impl Default for PEB {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[cfg(feature = "Win32_System_Kernel")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PEB_LDR_DATA {
    pub Reserved1: [u8; 8],
    pub Reserved2: [*mut core::ffi::c_void; 3],
    pub InMemoryOrderModuleList: super::Kernel::LIST_ENTRY,
}
#[cfg(feature = "Win32_System_Kernel")]
impl windows_core::TypeKind for PEB_LDR_DATA {
    type TypeKind = windows_core::CopyType;
}
#[cfg(feature = "Win32_System_Kernel")]
impl Default for PEB_LDR_DATA {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[cfg(feature = "Win32_System_Kernel")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PROCESS_BASIC_INFORMATION {
    pub ExitStatus: super::super::Foundation::NTSTATUS,
    pub PebBaseAddress: *mut PEB,
    pub AffinityMask: usize,
    pub BasePriority: i32,
    pub UniqueProcessId: usize,
    pub InheritedFromUniqueProcessId: usize,
}
#[cfg(feature = "Win32_System_Kernel")]
impl windows_core::TypeKind for PROCESS_BASIC_INFORMATION {
    type TypeKind = windows_core::CopyType;
}
#[cfg(feature = "Win32_System_Kernel")]
impl Default for PROCESS_BASIC_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RTL_USER_PROCESS_PARAMETERS {
    pub Reserved1: [u8; 16],
    pub Reserved2: [*mut core::ffi::c_void; 10],
    pub ImagePathName: super::super::Foundation::UNICODE_STRING,
    pub CommandLine: super::super::Foundation::UNICODE_STRING,
}
impl windows_core::TypeKind for RTL_USER_PROCESS_PARAMETERS {
    type TypeKind = windows_core::CopyType;
}
impl Default for RTL_USER_PROCESS_PARAMETERS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type PPS_POST_PROCESS_INIT_ROUTINE = Option<unsafe extern "system" fn()>;
pub type WAITORTIMERCALLBACK = Option<
    unsafe extern "system" fn(
        param0: *mut core::ffi::c_void,
        param1: super::super::Foundation::BOOLEAN,
    ),
>;
#[cfg(any())]
core::include!("impl.rs");
