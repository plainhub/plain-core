#[inline]
pub unsafe fn GetProcessHeap() -> windows_core::Result<super::super::Foundation::HANDLE> {
    windows_targets :: link ! ("kernel32.dll" "system" fn GetProcessHeap () -> super :: super :: Foundation :: HANDLE);
    let result__ = GetProcessHeap();
    (!result__.is_invalid())
        .then(|| result__)
        .ok_or_else(windows_core::Error::from_win32)
}
#[inline]
pub unsafe fn HeapAlloc<P0>(
    hheap: P0,
    dwflags: HEAP_FLAGS,
    dwbytes: usize,
) -> *mut core::ffi::c_void
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn HeapAlloc (hheap : super :: super :: Foundation :: HANDLE , dwflags : HEAP_FLAGS , dwbytes : usize) -> * mut core :: ffi :: c_void);
    HeapAlloc(hheap.param().abi(), dwflags, dwbytes)
}
#[inline]
pub unsafe fn HeapFree<P0>(
    hheap: P0,
    dwflags: HEAP_FLAGS,
    lpmem: Option<*const core::ffi::c_void>,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn HeapFree (hheap : super :: super :: Foundation :: HANDLE , dwflags : HEAP_FLAGS , lpmem : * const core :: ffi :: c_void) -> super :: super :: Foundation :: BOOL);
    HeapFree(
        hheap.param().abi(),
        dwflags,
        core::mem::transmute(lpmem.unwrap_or(std::ptr::null())),
    )
    .ok()
}
#[inline]
pub unsafe fn VirtualQueryEx<P0>(
    hprocess: P0,
    lpaddress: Option<*const core::ffi::c_void>,
    lpbuffer: *mut MEMORY_BASIC_INFORMATION,
    dwlength: usize,
) -> usize
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn VirtualQueryEx (hprocess : super :: super :: Foundation :: HANDLE , lpaddress : * const core :: ffi :: c_void , lpbuffer : * mut MEMORY_BASIC_INFORMATION , dwlength : usize) -> usize);
    VirtualQueryEx(
        hprocess.param().abi(),
        core::mem::transmute(lpaddress.unwrap_or(std::ptr::null())),
        lpbuffer,
        dwlength,
    )
}
pub const HEAP_ZERO_MEMORY: HEAP_FLAGS = HEAP_FLAGS(8u32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct HEAP_FLAGS(pub u32);
impl windows_core::TypeKind for HEAP_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for HEAP_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("HEAP_FLAGS").field(&self.0).finish()
    }
}
impl HEAP_FLAGS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for HEAP_FLAGS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for HEAP_FLAGS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for HEAP_FLAGS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for HEAP_FLAGS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for HEAP_FLAGS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PAGE_PROTECTION_FLAGS(pub u32);
impl windows_core::TypeKind for PAGE_PROTECTION_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PAGE_PROTECTION_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PAGE_PROTECTION_FLAGS")
            .field(&self.0)
            .finish()
    }
}
impl PAGE_PROTECTION_FLAGS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for PAGE_PROTECTION_FLAGS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for PAGE_PROTECTION_FLAGS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for PAGE_PROTECTION_FLAGS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for PAGE_PROTECTION_FLAGS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for PAGE_PROTECTION_FLAGS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PAGE_TYPE(pub u32);
impl windows_core::TypeKind for PAGE_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PAGE_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PAGE_TYPE").field(&self.0).finish()
    }
}
impl PAGE_TYPE {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for PAGE_TYPE {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for PAGE_TYPE {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for PAGE_TYPE {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for PAGE_TYPE {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for PAGE_TYPE {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct VIRTUAL_ALLOCATION_TYPE(pub u32);
impl windows_core::TypeKind for VIRTUAL_ALLOCATION_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for VIRTUAL_ALLOCATION_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("VIRTUAL_ALLOCATION_TYPE")
            .field(&self.0)
            .finish()
    }
}
impl VIRTUAL_ALLOCATION_TYPE {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for VIRTUAL_ALLOCATION_TYPE {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for VIRTUAL_ALLOCATION_TYPE {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for VIRTUAL_ALLOCATION_TYPE {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for VIRTUAL_ALLOCATION_TYPE {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for VIRTUAL_ALLOCATION_TYPE {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(C)]
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MEMORY_BASIC_INFORMATION {
    pub BaseAddress: *mut core::ffi::c_void,
    pub AllocationBase: *mut core::ffi::c_void,
    pub AllocationProtect: PAGE_PROTECTION_FLAGS,
    pub PartitionId: u16,
    pub RegionSize: usize,
    pub State: VIRTUAL_ALLOCATION_TYPE,
    pub Protect: PAGE_PROTECTION_FLAGS,
    pub Type: PAGE_TYPE,
}
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
impl windows_core::TypeKind for MEMORY_BASIC_INFORMATION {
    type TypeKind = windows_core::CopyType;
}
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
impl Default for MEMORY_BASIC_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MEMORY_BASIC_INFORMATION {
    pub BaseAddress: *mut core::ffi::c_void,
    pub AllocationBase: *mut core::ffi::c_void,
    pub AllocationProtect: PAGE_PROTECTION_FLAGS,
    pub RegionSize: usize,
    pub State: VIRTUAL_ALLOCATION_TYPE,
    pub Protect: PAGE_PROTECTION_FLAGS,
    pub Type: PAGE_TYPE,
}
#[cfg(target_arch = "x86")]
impl windows_core::TypeKind for MEMORY_BASIC_INFORMATION {
    type TypeKind = windows_core::CopyType;
}
#[cfg(target_arch = "x86")]
impl Default for MEMORY_BASIC_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
