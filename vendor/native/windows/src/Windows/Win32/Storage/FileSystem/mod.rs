#[cfg(feature = "Win32_Security")]
#[inline]
pub unsafe fn CreateFileW<P0, P1>(
    lpfilename: P0,
    dwdesiredaccess: u32,
    dwsharemode: FILE_SHARE_MODE,
    lpsecurityattributes: Option<*const super::super::Security::SECURITY_ATTRIBUTES>,
    dwcreationdisposition: FILE_CREATION_DISPOSITION,
    dwflagsandattributes: FILE_FLAGS_AND_ATTRIBUTES,
    htemplatefile: P1,
) -> windows_core::Result<super::super::Foundation::HANDLE>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
    P1: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn CreateFileW (lpfilename : windows_core :: PCWSTR , dwdesiredaccess : u32 , dwsharemode : FILE_SHARE_MODE , lpsecurityattributes : * const super :: super :: Security :: SECURITY_ATTRIBUTES , dwcreationdisposition : FILE_CREATION_DISPOSITION , dwflagsandattributes : FILE_FLAGS_AND_ATTRIBUTES , htemplatefile : super :: super :: Foundation :: HANDLE) -> super :: super :: Foundation :: HANDLE);
    let result__ = CreateFileW(
        lpfilename.param().abi(),
        dwdesiredaccess,
        dwsharemode,
        core::mem::transmute(lpsecurityattributes.unwrap_or(std::ptr::null())),
        dwcreationdisposition,
        dwflagsandattributes,
        htemplatefile.param().abi(),
    );
    (!result__.is_invalid())
        .then(|| result__)
        .ok_or_else(windows_core::Error::from_win32)
}
#[inline]
pub unsafe fn FindFirstVolumeW(
    lpszvolumename: &mut [u16],
) -> windows_core::Result<super::super::Foundation::HANDLE> {
    windows_targets :: link ! ("kernel32.dll" "system" fn FindFirstVolumeW (lpszvolumename : windows_core :: PWSTR , cchbufferlength : u32) -> super :: super :: Foundation :: HANDLE);
    let result__ = FindFirstVolumeW(
        core::mem::transmute(lpszvolumename.as_ptr()),
        lpszvolumename.len().try_into().unwrap(),
    );
    (!result__.is_invalid())
        .then(|| result__)
        .ok_or_else(windows_core::Error::from_win32)
}
#[inline]
pub unsafe fn FindNextVolumeW<P0>(
    hfindvolume: P0,
    lpszvolumename: &mut [u16],
) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn FindNextVolumeW (hfindvolume : super :: super :: Foundation :: HANDLE , lpszvolumename : windows_core :: PWSTR , cchbufferlength : u32) -> super :: super :: Foundation :: BOOL);
    FindNextVolumeW(
        hfindvolume.param().abi(),
        core::mem::transmute(lpszvolumename.as_ptr()),
        lpszvolumename.len().try_into().unwrap(),
    )
    .ok()
}
#[inline]
pub unsafe fn FindVolumeClose<P0>(hfindvolume: P0) -> windows_core::Result<()>
where
    P0: windows_core::Param<super::super::Foundation::HANDLE>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn FindVolumeClose (hfindvolume : super :: super :: Foundation :: HANDLE) -> super :: super :: Foundation :: BOOL);
    FindVolumeClose(hfindvolume.param().abi()).ok()
}
#[inline]
pub unsafe fn GetDiskFreeSpaceExW<P0>(
    lpdirectoryname: P0,
    lpfreebytesavailabletocaller: Option<*mut u64>,
    lptotalnumberofbytes: Option<*mut u64>,
    lptotalnumberoffreebytes: Option<*mut u64>,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetDiskFreeSpaceExW (lpdirectoryname : windows_core :: PCWSTR , lpfreebytesavailabletocaller : * mut u64 , lptotalnumberofbytes : * mut u64 , lptotalnumberoffreebytes : * mut u64) -> super :: super :: Foundation :: BOOL);
    GetDiskFreeSpaceExW(
        lpdirectoryname.param().abi(),
        core::mem::transmute(lpfreebytesavailabletocaller.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lptotalnumberofbytes.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lptotalnumberoffreebytes.unwrap_or(std::ptr::null_mut())),
    )
    .ok()
}
#[inline]
pub unsafe fn GetDriveTypeW<P0>(lprootpathname: P0) -> u32
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetDriveTypeW (lprootpathname : windows_core :: PCWSTR) -> u32);
    GetDriveTypeW(lprootpathname.param().abi())
}
#[inline]
pub unsafe fn GetVolumeInformationW<P0>(
    lprootpathname: P0,
    lpvolumenamebuffer: Option<&mut [u16]>,
    lpvolumeserialnumber: Option<*mut u32>,
    lpmaximumcomponentlength: Option<*mut u32>,
    lpfilesystemflags: Option<*mut u32>,
    lpfilesystemnamebuffer: Option<&mut [u16]>,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetVolumeInformationW (lprootpathname : windows_core :: PCWSTR , lpvolumenamebuffer : windows_core :: PWSTR , nvolumenamesize : u32 , lpvolumeserialnumber : * mut u32 , lpmaximumcomponentlength : * mut u32 , lpfilesystemflags : * mut u32 , lpfilesystemnamebuffer : windows_core :: PWSTR , nfilesystemnamesize : u32) -> super :: super :: Foundation :: BOOL);
    GetVolumeInformationW(
        lprootpathname.param().abi(),
        core::mem::transmute(
            lpvolumenamebuffer
                .as_deref()
                .map_or(core::ptr::null(), |slice| slice.as_ptr()),
        ),
        lpvolumenamebuffer
            .as_deref()
            .map_or(0, |slice| slice.len().try_into().unwrap()),
        core::mem::transmute(lpvolumeserialnumber.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpmaximumcomponentlength.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(lpfilesystemflags.unwrap_or(std::ptr::null_mut())),
        core::mem::transmute(
            lpfilesystemnamebuffer
                .as_deref()
                .map_or(core::ptr::null(), |slice| slice.as_ptr()),
        ),
        lpfilesystemnamebuffer
            .as_deref()
            .map_or(0, |slice| slice.len().try_into().unwrap()),
    )
    .ok()
}
#[inline]
pub unsafe fn GetVolumePathNamesForVolumeNameW<P0>(
    lpszvolumename: P0,
    lpszvolumepathnames: Option<&mut [u16]>,
    lpcchreturnlength: *mut u32,
) -> windows_core::Result<()>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_targets :: link ! ("kernel32.dll" "system" fn GetVolumePathNamesForVolumeNameW (lpszvolumename : windows_core :: PCWSTR , lpszvolumepathnames : windows_core :: PWSTR , cchbufferlength : u32 , lpcchreturnlength : * mut u32) -> super :: super :: Foundation :: BOOL);
    GetVolumePathNamesForVolumeNameW(
        lpszvolumename.param().abi(),
        core::mem::transmute(
            lpszvolumepathnames
                .as_deref()
                .map_or(core::ptr::null(), |slice| slice.as_ptr()),
        ),
        lpszvolumepathnames
            .as_deref()
            .map_or(0, |slice| slice.len().try_into().unwrap()),
        lpcchreturnlength,
    )
    .ok()
}
pub const FILE_SHARE_READ: FILE_SHARE_MODE = FILE_SHARE_MODE(1u32);
pub const FILE_SHARE_WRITE: FILE_SHARE_MODE = FILE_SHARE_MODE(2u32);
pub const OPEN_EXISTING: FILE_CREATION_DISPOSITION = FILE_CREATION_DISPOSITION(3u32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct FILE_ACCESS_RIGHTS(pub u32);
impl windows_core::TypeKind for FILE_ACCESS_RIGHTS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for FILE_ACCESS_RIGHTS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("FILE_ACCESS_RIGHTS").field(&self.0).finish()
    }
}
impl FILE_ACCESS_RIGHTS {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for FILE_ACCESS_RIGHTS {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for FILE_ACCESS_RIGHTS {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for FILE_ACCESS_RIGHTS {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for FILE_ACCESS_RIGHTS {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for FILE_ACCESS_RIGHTS {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct FILE_CREATION_DISPOSITION(pub u32);
impl windows_core::TypeKind for FILE_CREATION_DISPOSITION {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for FILE_CREATION_DISPOSITION {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("FILE_CREATION_DISPOSITION")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct FILE_FLAGS_AND_ATTRIBUTES(pub u32);
impl windows_core::TypeKind for FILE_FLAGS_AND_ATTRIBUTES {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for FILE_FLAGS_AND_ATTRIBUTES {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("FILE_FLAGS_AND_ATTRIBUTES")
            .field(&self.0)
            .finish()
    }
}
impl FILE_FLAGS_AND_ATTRIBUTES {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for FILE_FLAGS_AND_ATTRIBUTES {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for FILE_FLAGS_AND_ATTRIBUTES {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for FILE_FLAGS_AND_ATTRIBUTES {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for FILE_FLAGS_AND_ATTRIBUTES {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for FILE_FLAGS_AND_ATTRIBUTES {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct FILE_SHARE_MODE(pub u32);
impl windows_core::TypeKind for FILE_SHARE_MODE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for FILE_SHARE_MODE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("FILE_SHARE_MODE").field(&self.0).finish()
    }
}
impl FILE_SHARE_MODE {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for FILE_SHARE_MODE {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for FILE_SHARE_MODE {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for FILE_SHARE_MODE {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for FILE_SHARE_MODE {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for FILE_SHARE_MODE {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[cfg(any())]
core::include!("impl.rs");
