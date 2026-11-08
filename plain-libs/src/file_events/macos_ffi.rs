// Adapted from fsevent-sys 4.1.0 (fsevent-sys/SOURCE.json).
use std::ffi::c_void;

pub(super) type FSEventStreamRef = *mut c_void;
pub(super) type FSEventStreamEventFlags = u32;
pub(super) type FSEventStreamEventId = u64;

type FSEventStreamCallback = extern "C" fn(
    *const c_void,
    *mut c_void,
    usize,
    *mut c_void,
    *const FSEventStreamEventFlags,
    *const FSEventStreamEventId,
);

pub(super) const EVENT_ID_SINCE_NOW: u64 = 0xFFFFFFFFFFFFFFFF;
pub(super) const CREATE_FLAG_NO_DEFER: u32 = 0x00000002;
pub(super) const CREATE_FLAG_FILE_EVENTS: u32 = 0x00000010;

#[repr(C)]
pub(super) struct FSEventStreamContext {
    pub(super) version: isize,
    pub(super) info: *mut c_void,
    pub(super) retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
    pub(super) release: Option<extern "C" fn(*const c_void)>,
    pub(super) copy_description: Option<extern "C" fn(*const c_void) -> *const c_void>,
}

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    pub(super) fn FSEventStreamCreate(
        allocator: *const c_void,
        callback: FSEventStreamCallback,
        context: *mut FSEventStreamContext,
        paths: *const c_void,
        since: FSEventStreamEventId,
        latency: f64,
        flags: u32,
    ) -> FSEventStreamRef;
    pub(super) fn FSEventStreamSetDispatchQueue(stream: FSEventStreamRef, queue: *mut c_void);
    pub(super) fn FSEventStreamStart(stream: FSEventStreamRef) -> u8;
    pub(super) fn FSEventStreamStop(stream: FSEventStreamRef);
    pub(super) fn FSEventStreamInvalidate(stream: FSEventStreamRef);
    pub(super) fn FSEventStreamRelease(stream: FSEventStreamRef);
}

pub(super) const CF_COMPARE_CASE_INSENSITIVE: usize = 1;

#[repr(C)]
pub(super) struct CFArrayCallBacks {
    version: isize,
    retain: Option<extern "C" fn(*const c_void, *const c_void) -> *const c_void>,
    release: Option<extern "C" fn(*const c_void, *const c_void)>,
    copy_description: Option<extern "C" fn(*const c_void) -> *const c_void>,
    equal: Option<extern "C" fn(*const c_void, *const c_void) -> u8>,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub(super) static kCFTypeArrayCallBacks: CFArrayCallBacks;
    pub(super) fn CFArrayCreateMutable(
        allocator: *const c_void,
        capacity: isize,
        callbacks: *const CFArrayCallBacks,
    ) -> *mut c_void;
    pub(super) fn CFArrayAppendValue(array: *mut c_void, value: *const c_void);
    pub(super) fn CFArrayRemoveValueAtIndex(array: *mut c_void, index: isize);
    pub(super) fn CFStringCompare(
        left: *const c_void,
        right: *const c_void,
        options: usize,
    ) -> isize;
}
