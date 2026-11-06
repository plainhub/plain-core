#[inline]
pub unsafe fn DeviceIoControl<P0>(
    hdevice: P0,
    dwiocontrolcode: u32,
    lpinbuffer: Option<*const core::ffi::c_void>,
    ninbuffersize: u32,
    lpoutbuffer: Option<*mut core::ffi::c_void>,
    noutbuffersize: u32,
    lpbytesreturned: Option<*mut u32>,
    lpoverlapped: Option<*mut OVERLAPPED>,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn DeviceIoControl (hdevice : super :: super :: Foundation :: HANDLE , dwiocontrolcode : u32 , lpinbuffer : * const core :: ffi :: c_void , ninbuffersize : u32 , lpoutbuffer : * mut core :: ffi :: c_void , noutbuffersize : u32 , lpbytesreturned : * mut u32 , lpoverlapped : * mut OVERLAPPED) -> super :: super :: Foundation :: BOOL);
    DeviceIoControl(
        hdevice.param().abi(),
        dwiocontrolcode,
        core::mem::transmute(lpinbuffer.unwrap_or(std::ptr::null())),
        ninbuffersize,
        core::mem::transmute(lpoutbuffer.unwrap_or(std::ptr::null_mut())),
        noutbuffersize,
        core::mem::transmute(lpbytesreturned.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpoverlapped.unwrap_or(std::ptr::null_mut())),
    )
    .ok()
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OVERLAPPED {
    pub Internal: usize,
    pub InternalHigh: usize,
    pub Anonymous: OVERLAPPED_0,
    pub hEvent: super::super::Foundation::HANDLE,
}
impl windows_core::TypeKind for OVERLAPPED {
    type TypeKind = windows_core::CopyType;
}
impl Default for OVERLAPPED {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union OVERLAPPED_0 {
    pub Anonymous: OVERLAPPED_0_0,
    pub Pointer: *mut core::ffi::c_void,
}
impl windows_core::TypeKind for OVERLAPPED_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for OVERLAPPED_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OVERLAPPED_0_0 {
    pub Offset: u32,
    pub OffsetHigh: u32,
}
impl windows_core::TypeKind for OVERLAPPED_0_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for OVERLAPPED_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
