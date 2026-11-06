#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LIST_ENTRY {
    pub Flink: *mut LIST_ENTRY,
    pub Blink: *mut LIST_ENTRY,
}
impl windows_core::TypeKind for LIST_ENTRY {
    type TypeKind = windows_core::CopyType;
}
impl Default for LIST_ENTRY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
