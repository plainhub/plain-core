#[inline]
pub unsafe fn GetComputerNameExW(
    nametype: COMPUTER_NAME_FORMAT,
    lpbuffer: windows_core::PWSTR,
    nsize: *mut u32,
) -> windows_core::Result<()> {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetComputerNameExW (nametype : COMPUTER_NAME_FORMAT , lpbuffer : windows_core :: PWSTR , nsize : * mut u32) -> super :: super :: Foundation :: BOOL);
    GetComputerNameExW(nametype, core::mem::transmute(lpbuffer), nsize).ok()
}
#[inline]
pub unsafe fn GetLogicalProcessorInformationEx(
    relationshiptype: LOGICAL_PROCESSOR_RELATIONSHIP,
    buffer: Option<*mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>,
    returnedlength: *mut u32,
) -> windows_core::Result<()> {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetLogicalProcessorInformationEx (relationshiptype : LOGICAL_PROCESSOR_RELATIONSHIP , buffer : * mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX , returnedlength : * mut u32) -> super :: super :: Foundation :: BOOL);
    GetLogicalProcessorInformationEx(
        relationshiptype,
        core::mem::transmute(buffer.unwrap_or(std::ptr::null_mut())),
        returnedlength,
    )
    .ok()
}
#[inline]
pub unsafe fn GetSystemInfo(lpsysteminfo: *mut SYSTEM_INFO) {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetSystemInfo (lpsysteminfo : * mut SYSTEM_INFO));
    GetSystemInfo(lpsysteminfo)
}
#[inline]
pub unsafe fn GetTickCount64() -> u64 {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetTickCount64 () -> u64);
    GetTickCount64()
}
#[inline]
pub unsafe fn GlobalMemoryStatusEx(lpbuffer: *mut MEMORYSTATUSEX) -> windows_core::Result<()> {
    windows_targets :: link ! ("kernel32.dll" "system" fn GlobalMemoryStatusEx (lpbuffer : * mut MEMORYSTATUSEX) -> super :: super :: Foundation :: BOOL);
    GlobalMemoryStatusEx(lpbuffer).ok()
}
pub const ComputerNamePhysicalDnsHostname: COMPUTER_NAME_FORMAT = COMPUTER_NAME_FORMAT(5i32);
pub const PROCESSOR_ARCHITECTURE_ALPHA: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(2u16);
pub const PROCESSOR_ARCHITECTURE_ALPHA64: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(7u16);
pub const PROCESSOR_ARCHITECTURE_AMD64: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(9u16);
pub const PROCESSOR_ARCHITECTURE_ARM: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(5u16);
pub const PROCESSOR_ARCHITECTURE_ARM32_ON_WIN64: PROCESSOR_ARCHITECTURE =
    PROCESSOR_ARCHITECTURE(13u16);
pub const PROCESSOR_ARCHITECTURE_ARM64: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(12u16);
pub const PROCESSOR_ARCHITECTURE_IA32_ON_ARM64: PROCESSOR_ARCHITECTURE =
    PROCESSOR_ARCHITECTURE(14u16);
pub const PROCESSOR_ARCHITECTURE_IA32_ON_WIN64: PROCESSOR_ARCHITECTURE =
    PROCESSOR_ARCHITECTURE(10u16);
pub const PROCESSOR_ARCHITECTURE_IA64: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(6u16);
pub const PROCESSOR_ARCHITECTURE_INTEL: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(0u16);
pub const PROCESSOR_ARCHITECTURE_MIPS: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(1u16);
pub const PROCESSOR_ARCHITECTURE_MSIL: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(8u16);
pub const PROCESSOR_ARCHITECTURE_NEUTRAL: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(11u16);
pub const PROCESSOR_ARCHITECTURE_PPC: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(3u16);
pub const PROCESSOR_ARCHITECTURE_SHX: PROCESSOR_ARCHITECTURE = PROCESSOR_ARCHITECTURE(4u16);
pub const RelationAll: LOGICAL_PROCESSOR_RELATIONSHIP = LOGICAL_PROCESSOR_RELATIONSHIP(65535i32);
pub const RelationProcessorCore: LOGICAL_PROCESSOR_RELATIONSHIP =
    LOGICAL_PROCESSOR_RELATIONSHIP(0i32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct COMPUTER_NAME_FORMAT(pub i32);
impl windows_core::TypeKind for COMPUTER_NAME_FORMAT {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for COMPUTER_NAME_FORMAT {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("COMPUTER_NAME_FORMAT")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct LOGICAL_PROCESSOR_RELATIONSHIP(pub i32);
impl windows_core::TypeKind for LOGICAL_PROCESSOR_RELATIONSHIP {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for LOGICAL_PROCESSOR_RELATIONSHIP {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("LOGICAL_PROCESSOR_RELATIONSHIP")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PROCESSOR_ARCHITECTURE(pub u16);
impl windows_core::TypeKind for PROCESSOR_ARCHITECTURE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PROCESSOR_ARCHITECTURE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PROCESSOR_ARCHITECTURE")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PROCESSOR_CACHE_TYPE(pub i32);
impl windows_core::TypeKind for PROCESSOR_CACHE_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PROCESSOR_CACHE_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PROCESSOR_CACHE_TYPE")
            .field(&self.0)
            .finish()
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CACHE_RELATIONSHIP {
    pub Level: u8,
    pub Associativity: u8,
    pub LineSize: u16,
    pub CacheSize: u32,
    pub Type: PROCESSOR_CACHE_TYPE,
    pub Reserved: [u8; 18],
    pub GroupCount: u16,
    pub Anonymous: CACHE_RELATIONSHIP_0,
}
impl windows_core::TypeKind for CACHE_RELATIONSHIP {
    type TypeKind = windows_core::CopyType;
}
impl Default for CACHE_RELATIONSHIP {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union CACHE_RELATIONSHIP_0 {
    pub GroupMask: GROUP_AFFINITY,
    pub GroupMasks: [GROUP_AFFINITY; 1],
}
impl windows_core::TypeKind for CACHE_RELATIONSHIP_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for CACHE_RELATIONSHIP_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GROUP_AFFINITY {
    pub Mask: usize,
    pub Group: u16,
    pub Reserved: [u16; 3],
}
impl windows_core::TypeKind for GROUP_AFFINITY {
    type TypeKind = windows_core::CopyType;
}
impl Default for GROUP_AFFINITY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GROUP_RELATIONSHIP {
    pub MaximumGroupCount: u16,
    pub ActiveGroupCount: u16,
    pub Reserved: [u8; 20],
    pub GroupInfo: [PROCESSOR_GROUP_INFO; 1],
}
impl windows_core::TypeKind for GROUP_RELATIONSHIP {
    type TypeKind = windows_core::CopyType;
}
impl Default for GROUP_RELATIONSHIP {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MEMORYSTATUSEX {
    pub dwLength: u32,
    pub dwMemoryLoad: u32,
    pub ullTotalPhys: u64,
    pub ullAvailPhys: u64,
    pub ullTotalPageFile: u64,
    pub ullAvailPageFile: u64,
    pub ullTotalVirtual: u64,
    pub ullAvailVirtual: u64,
    pub ullAvailExtendedVirtual: u64,
}
impl windows_core::TypeKind for MEMORYSTATUSEX {
    type TypeKind = windows_core::CopyType;
}
impl Default for MEMORYSTATUSEX {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NUMA_NODE_RELATIONSHIP {
    pub NodeNumber: u32,
    pub Reserved: [u8; 18],
    pub GroupCount: u16,
    pub Anonymous: NUMA_NODE_RELATIONSHIP_0,
}
impl windows_core::TypeKind for NUMA_NODE_RELATIONSHIP {
    type TypeKind = windows_core::CopyType;
}
impl Default for NUMA_NODE_RELATIONSHIP {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union NUMA_NODE_RELATIONSHIP_0 {
    pub GroupMask: GROUP_AFFINITY,
    pub GroupMasks: [GROUP_AFFINITY; 1],
}
impl windows_core::TypeKind for NUMA_NODE_RELATIONSHIP_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for NUMA_NODE_RELATIONSHIP_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OSVERSIONINFOEXW {
    pub dwOSVersionInfoSize: u32,
    pub dwMajorVersion: u32,
    pub dwMinorVersion: u32,
    pub dwBuildNumber: u32,
    pub dwPlatformId: u32,
    pub szCSDVersion: [u16; 128],
    pub wServicePackMajor: u16,
    pub wServicePackMinor: u16,
    pub wSuiteMask: u16,
    pub wProductType: u8,
    pub wReserved: u8,
}
impl windows_core::TypeKind for OSVERSIONINFOEXW {
    type TypeKind = windows_core::CopyType;
}
impl Default for OSVERSIONINFOEXW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OSVERSIONINFOW {
    pub dwOSVersionInfoSize: u32,
    pub dwMajorVersion: u32,
    pub dwMinorVersion: u32,
    pub dwBuildNumber: u32,
    pub dwPlatformId: u32,
    pub szCSDVersion: [u16; 128],
}
impl windows_core::TypeKind for OSVERSIONINFOW {
    type TypeKind = windows_core::CopyType;
}
impl Default for OSVERSIONINFOW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PROCESSOR_GROUP_INFO {
    pub MaximumProcessorCount: u8,
    pub ActiveProcessorCount: u8,
    pub Reserved: [u8; 38],
    pub ActiveProcessorMask: usize,
}
impl windows_core::TypeKind for PROCESSOR_GROUP_INFO {
    type TypeKind = windows_core::CopyType;
}
impl Default for PROCESSOR_GROUP_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PROCESSOR_RELATIONSHIP {
    pub Flags: u8,
    pub EfficiencyClass: u8,
    pub Reserved: [u8; 20],
    pub GroupCount: u16,
    pub GroupMask: [GROUP_AFFINITY; 1],
}
impl windows_core::TypeKind for PROCESSOR_RELATIONSHIP {
    type TypeKind = windows_core::CopyType;
}
impl Default for PROCESSOR_RELATIONSHIP {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SYSTEM_INFO {
    pub Anonymous: SYSTEM_INFO_0,
    pub dwPageSize: u32,
    pub lpMinimumApplicationAddress: *mut core::ffi::c_void,
    pub lpMaximumApplicationAddress: *mut core::ffi::c_void,
    pub dwActiveProcessorMask: usize,
    pub dwNumberOfProcessors: u32,
    pub dwProcessorType: u32,
    pub dwAllocationGranularity: u32,
    pub wProcessorLevel: u16,
    pub wProcessorRevision: u16,
}
impl windows_core::TypeKind for SYSTEM_INFO {
    type TypeKind = windows_core::CopyType;
}
impl Default for SYSTEM_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union SYSTEM_INFO_0 {
    pub dwOemId: u32,
    pub Anonymous: SYSTEM_INFO_0_0,
}
impl windows_core::TypeKind for SYSTEM_INFO_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for SYSTEM_INFO_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SYSTEM_INFO_0_0 {
    pub wProcessorArchitecture: PROCESSOR_ARCHITECTURE,
    pub wReserved: u16,
}
impl windows_core::TypeKind for SYSTEM_INFO_0_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for SYSTEM_INFO_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX {
    pub Relationship: LOGICAL_PROCESSOR_RELATIONSHIP,
    pub Size: u32,
    pub Anonymous: SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX_0,
}
impl windows_core::TypeKind for SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX {
    type TypeKind = windows_core::CopyType;
}
impl Default for SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX_0 {
    pub Processor: PROCESSOR_RELATIONSHIP,
    pub NumaNode: NUMA_NODE_RELATIONSHIP,
    pub Cache: CACHE_RELATIONSHIP,
    pub Group: GROUP_RELATIONSHIP,
}
impl windows_core::TypeKind for SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
