pub const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 2954240u32;
pub const PropertyStandardQuery: STORAGE_QUERY_TYPE = STORAGE_QUERY_TYPE(0i32);
pub const StorageDeviceSeekPenaltyProperty: STORAGE_PROPERTY_ID = STORAGE_PROPERTY_ID(7i32);
pub const Unknown: MEDIA_TYPE = MEDIA_TYPE(0i32);
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MEDIA_TYPE(pub i32);
impl windows_core::TypeKind for MEDIA_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MEDIA_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MEDIA_TYPE").field(&self.0).finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct STORAGE_PROPERTY_ID(pub i32);
impl windows_core::TypeKind for STORAGE_PROPERTY_ID {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for STORAGE_PROPERTY_ID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("STORAGE_PROPERTY_ID").field(&self.0).finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct STORAGE_QUERY_TYPE(pub i32);
impl windows_core::TypeKind for STORAGE_QUERY_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for STORAGE_QUERY_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("STORAGE_QUERY_TYPE").field(&self.0).finish()
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DEVICE_SEEK_PENALTY_DESCRIPTOR {
    pub Version: u32,
    pub Size: u32,
    pub IncursSeekPenalty: super::super::Foundation::BOOLEAN,
}
impl windows_core::TypeKind for DEVICE_SEEK_PENALTY_DESCRIPTOR {
    type TypeKind = windows_core::CopyType;
}
impl Default for DEVICE_SEEK_PENALTY_DESCRIPTOR {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STORAGE_PROPERTY_QUERY {
    pub PropertyId: STORAGE_PROPERTY_ID,
    pub QueryType: STORAGE_QUERY_TYPE,
    pub AdditionalParameters: [u8; 1],
}
impl windows_core::TypeKind for STORAGE_PROPERTY_QUERY {
    type TypeKind = windows_core::CopyType;
}
impl Default for STORAGE_PROPERTY_QUERY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
