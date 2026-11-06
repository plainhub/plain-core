pub const DRIVE_FIXED: u32 = 3u32;
pub const DRIVE_REMOVABLE: u32 = 2u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SYSTEM_PROCESS_INFORMATION {
    pub NextEntryOffset: u32,
    pub NumberOfThreads: u32,
    pub Reserved1: [u8; 48],
    pub ImageName: super::super::Foundation::UNICODE_STRING,
    pub BasePriority: i32,
    pub UniqueProcessId: super::super::Foundation::HANDLE,
    pub Reserved2: *mut core::ffi::c_void,
    pub HandleCount: u32,
    pub SessionId: u32,
    pub Reserved3: *mut core::ffi::c_void,
    pub PeakVirtualSize: usize,
    pub VirtualSize: usize,
    pub Reserved4: u32,
    pub PeakWorkingSetSize: usize,
    pub WorkingSetSize: usize,
    pub Reserved5: *mut core::ffi::c_void,
    pub QuotaPagedPoolUsage: usize,
    pub Reserved6: *mut core::ffi::c_void,
    pub QuotaNonPagedPoolUsage: usize,
    pub PagefileUsage: usize,
    pub PeakPagefileUsage: usize,
    pub PrivatePageCount: usize,
    pub Reserved7: [i64; 6],
}
impl windows_core::TypeKind for SYSTEM_PROCESS_INFORMATION {
    type TypeKind = windows_core::CopyType;
}
impl Default for SYSTEM_PROCESS_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[cfg(any())]
core::include!("impl.rs");
