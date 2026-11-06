#[inline]
pub unsafe fn GetModuleFileNameExW<P0, P1>(hprocess: P0, hmodule: P1, lpfilename: &mut [u16]) -> u32
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
    P1: windows_core::Param<super::super::Foundation::HMODULE>,
{
    windows_targets :: link ! ("psapi.dll" "system" fn GetModuleFileNameExW (hprocess : super :: super :: Foundation :: HANDLE , hmodule : super :: super :: Foundation :: HMODULE , lpfilename : windows_core :: PWSTR , nsize : u32) -> u32);
    GetModuleFileNameExW(
        hprocess.param().abi(),
        hmodule.param().abi(),
        core::mem::transmute(lpfilename.as_ptr()),
        lpfilename.len().try_into().unwrap(),
    )
}
#[inline]
pub unsafe fn K32GetPerformanceInfo(
    pperformanceinformation: *mut PERFORMANCE_INFORMATION,
    cb: u32,
) -> super::super::Foundation::BOOL {
    windows_targets :: link ! ("kernel32.dll" "system" fn K32GetPerformanceInfo (pperformanceinformation : * mut PERFORMANCE_INFORMATION , cb : u32) -> super :: super :: Foundation :: BOOL);
    K32GetPerformanceInfo(pperformanceinformation, cb)
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PERFORMANCE_INFORMATION {
    pub cb: u32,
    pub CommitTotal: usize,
    pub CommitLimit: usize,
    pub CommitPeak: usize,
    pub PhysicalTotal: usize,
    pub PhysicalAvailable: usize,
    pub SystemCache: usize,
    pub KernelTotal: usize,
    pub KernelPaged: usize,
    pub KernelNonpaged: usize,
    pub PageSize: usize,
    pub HandleCount: u32,
    pub ProcessCount: u32,
    pub ThreadCount: u32,
}
impl windows_core::TypeKind for PERFORMANCE_INFORMATION {
    type TypeKind = windows_core::CopyType;
}
impl Default for PERFORMANCE_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
