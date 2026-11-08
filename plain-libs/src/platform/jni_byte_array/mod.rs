// ABI declarations adapted from jni-sys 0.4.1 (https://github.com/jni-rs/jni-sys), see LICENSE-MIT.

use std::{
    ffi::{CStr, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

pub type EnvPtr = *mut *const Functions;
pub type Object = *mut c_void;

#[repr(C)]
pub struct Functions {
    reserved_0_to_5: [*const c_void; 6],
    find_class: unsafe extern "system" fn(EnvPtr, *const c_char) -> Object,
    reserved_7_to_13: [*const c_void; 7],
    throw_new: unsafe extern "system" fn(EnvPtr, Object, *const c_char) -> i32,
    reserved_15_to_170: [*const c_void; 156],
    get_array_length: unsafe extern "system" fn(EnvPtr, Object) -> i32,
    reserved_172_to_175: [*const c_void; 4],
    new_byte_array: unsafe extern "system" fn(EnvPtr, i32) -> Object,
    reserved_177_to_199: [*const c_void; 23],
    get_byte_array_region: unsafe extern "system" fn(EnvPtr, Object, i32, i32, *mut i8),
    reserved_201_to_207: [*const c_void; 7],
    set_byte_array_region: unsafe extern "system" fn(EnvPtr, Object, i32, i32, *const i8),
    reserved_209_to_227: [*const c_void; 19],
    exception_check: unsafe extern "system" fn(EnvPtr) -> u8,
}

pub struct Env<'a> {
    raw: EnvPtr,
    functions: &'a Functions,
}

/// # Safety
/// The VM must supply `raw` for this thread and native call.
pub unsafe fn with_env<T: Default>(raw: EnvPtr, f: impl FnOnce(&Env<'_>) -> T) -> T {
    let env = Env {
        raw,
        functions: unsafe { &**raw },
    };
    match catch_unwind(AssertUnwindSafe(|| f(&env))) {
        Ok(value) => value,
        Err(payload) => {
            let message = match payload.downcast::<&'static str>() {
                Ok(text) => format!("Rust panic: {text}"),
                Err(payload) => {
                    if let Err(drop_panic) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                        std::mem::forget(drop_panic);
                    }
                    "Rust panic: non-string panic payload".to_owned()
                }
            };
            env.throw(c"java/lang/RuntimeException", &message);
            T::default()
        }
    }
}

impl Env<'_> {
    /// # Safety
    /// `array` must be null or a byte array reference from this VM and native call.
    pub unsafe fn read(&self, array: Object) -> Option<Vec<u8>> {
        if array.is_null() {
            self.throw(
                c"java/lang/RuntimeException",
                "Rust error: Null pointer in convert_byte_array array argument",
            );
            return None;
        }
        let len = unsafe { (self.functions.get_array_length)(self.raw, array) };
        let mut bytes = vec![0u8; len as usize];
        unsafe {
            (self.functions.get_byte_array_region)(
                self.raw,
                array,
                0,
                len,
                bytes.as_mut_ptr().cast(),
            );
        }
        (!self.exception_pending()).then_some(bytes)
    }

    pub fn write(&self, bytes: &[u8]) -> Object {
        let Ok(len) = i32::try_from(bytes.len()) else {
            self.throw(
                c"java/lang/RuntimeException",
                "Rust error: byte array too large",
            );
            return ptr::null_mut();
        };
        let array = unsafe { (self.functions.new_byte_array)(self.raw, len) };
        if array.is_null() {
            self.throw(
                c"java/lang/RuntimeException",
                "Rust error: byte array allocation failed",
            );
            return array;
        }
        unsafe {
            (self.functions.set_byte_array_region)(self.raw, array, 0, len, bytes.as_ptr().cast());
        }
        if self.exception_pending() {
            return ptr::null_mut();
        }
        array
    }

    pub fn illegal_state(&self, message: &str) {
        self.throw(c"java/lang/IllegalStateException", message);
    }

    fn throw(&self, class: &CStr, message: &str) {
        if self.exception_pending() {
            return;
        }
        let class = unsafe { (self.functions.find_class)(self.raw, class.as_ptr()) };
        if !class.is_null() {
            let bytes = modified_utf8(message);
            unsafe { (self.functions.throw_new)(self.raw, class, bytes.as_ptr().cast()) };
        }
    }

    fn exception_pending(&self) -> bool {
        unsafe { (self.functions.exception_check)(self.raw) != 0 }
    }
}

fn modified_utf8(message: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(message.len() + 1);
    for unit in message.encode_utf16() {
        match unit {
            1..=0x7f => bytes.push(unit as u8),
            0..=0x7ff => {
                bytes.push((0xc0 | unit >> 6) as u8);
                bytes.push((0x80 | unit & 0x3f) as u8);
            }
            _ => {
                bytes.push((0xe0 | unit >> 12) as u8);
                bytes.push((0x80 | (unit >> 6) & 0x3f) as u8);
                bytes.push((0x80 | unit & 0x3f) as u8);
            }
        }
    }
    bytes.push(0);
    bytes
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/platform/jni_byte_array.rs"]
mod tests;
