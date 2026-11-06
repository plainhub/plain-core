#[inline]
pub unsafe fn PdhAddEnglishCounterA<P0>(
    hquery: isize,
    szfullcounterpath: P0,
    dwuserdata: usize,
    phcounter: *mut isize,
) -> u32
where
    P0: windows_core::Param<windows_core::PCSTR>,
{
    windows_targets :: link ! ("pdh.dll" "system" fn PdhAddEnglishCounterA (hquery : isize , szfullcounterpath : windows_core :: PCSTR , dwuserdata : usize , phcounter : * mut isize) -> u32);
    PdhAddEnglishCounterA(
        hquery,
        szfullcounterpath.param().abi(),
        dwuserdata,
        phcounter,
    )
}
#[inline]
pub unsafe fn PdhAddEnglishCounterW<P0>(
    hquery: isize,
    szfullcounterpath: P0,
    dwuserdata: usize,
    phcounter: *mut isize,
) -> u32
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("pdh.dll" "system" fn PdhAddEnglishCounterW (hquery : isize , szfullcounterpath : windows_core :: PCWSTR , dwuserdata : usize , phcounter : * mut isize) -> u32);
    PdhAddEnglishCounterW(
        hquery,
        szfullcounterpath.param().abi(),
        dwuserdata,
        phcounter,
    )
}
#[inline]
pub unsafe fn PdhCloseQuery(hquery: isize) -> u32 {
    windows_targets :: link ! ("pdh.dll" "system" fn PdhCloseQuery (hquery : isize) -> u32);
    PdhCloseQuery(hquery)
}
#[inline]
pub unsafe fn PdhCollectQueryData(hquery: isize) -> u32 {
    windows_targets :: link ! ("pdh.dll" "system" fn PdhCollectQueryData (hquery : isize) -> u32);
    PdhCollectQueryData(hquery)
}
#[inline]
pub unsafe fn PdhCollectQueryDataEx<P0>(
    hquery: isize,
    dwintervaltime: u32,
    hnewdataevent: P0,
) -> u32
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("pdh.dll" "system" fn PdhCollectQueryDataEx (hquery : isize , dwintervaltime : u32 , hnewdataevent : super :: super :: Foundation :: HANDLE) -> u32);
    PdhCollectQueryDataEx(hquery, dwintervaltime, hnewdataevent.param().abi())
}
#[inline]
pub unsafe fn PdhGetFormattedCounterValue(
    hcounter: isize,
    dwformat: PDH_FMT,
    lpdwtype: Option<*mut u32>,
    pvalue: *mut PDH_FMT_COUNTERVALUE,
) -> u32 {
    windows_targets :: link ! ("pdh.dll" "system" fn PdhGetFormattedCounterValue (hcounter : isize , dwformat : PDH_FMT , lpdwtype : * mut u32 , pvalue : * mut PDH_FMT_COUNTERVALUE) -> u32);
    PdhGetFormattedCounterValue(
        hcounter,
        dwformat,
        core::mem::transmute(lpdwtype.unwrap_or(std::ptr::null_mut())),
        pvalue,
    )
}
#[inline]
pub unsafe fn PdhOpenQueryA<P0>(szdatasource: P0, dwuserdata: usize, phquery: *mut isize) -> u32
where
    P0: windows_core::Param<windows_core::PCSTR>,
{
    windows_targets :: link ! ("pdh.dll" "system" fn PdhOpenQueryA (szdatasource : windows_core :: PCSTR , dwuserdata : usize , phquery : * mut isize) -> u32);
    PdhOpenQueryA(szdatasource.param().abi(), dwuserdata, phquery)
}
#[inline]
pub unsafe fn PdhRemoveCounter(hcounter: isize) -> u32 {
    windows_targets :: link ! ("pdh.dll" "system" fn PdhRemoveCounter (hcounter : isize) -> u32);
    PdhRemoveCounter(hcounter)
}
pub const PDH_FMT_DOUBLE: PDH_FMT = PDH_FMT(512u32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct PDH_FMT(pub u32);
impl windows_core::TypeKind for PDH_FMT {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for PDH_FMT {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PDH_FMT").field(&self.0).finish()
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PDH_FMT_COUNTERVALUE {
    pub CStatus: u32,
    pub Anonymous: PDH_FMT_COUNTERVALUE_0,
}
impl windows_core::TypeKind for PDH_FMT_COUNTERVALUE {
    type TypeKind = windows_core::CopyType;
}
impl Default for PDH_FMT_COUNTERVALUE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union PDH_FMT_COUNTERVALUE_0 {
    pub longValue: i32,
    pub doubleValue: f64,
    pub largeValue: i64,
    pub AnsiStringValue: windows_core::PCSTR,
    pub WideStringValue: windows_core::PCWSTR,
}
impl windows_core::TypeKind for PDH_FMT_COUNTERVALUE_0 {
    type TypeKind = windows_core::CopyType;
}
impl Default for PDH_FMT_COUNTERVALUE_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[cfg(any())]
core::include!("impl.rs");
