use crate::infrastructure::protocols::ble as ble_wire;
use crate::infrastructure::protocols::ble::Assembler;
use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
static HANDLES: LazyLock<Mutex<HashMap<u64, Assembler>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
struct Encoder {
    data: Vec<u8>,
    id: u32,
    response: bool,
    sequence: u32,
    limit: usize,
}
static ENCODERS: LazyLock<Mutex<HashMap<u64, Encoder>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicU64 = AtomicU64::new(1);
#[unsafe(no_mangle)]
pub extern "C" fn plain_ble_new() -> u64 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    HANDLES.lock().unwrap().insert(id, Assembler::default());
    id
}
#[unsafe(no_mangle)]
pub extern "C" fn plain_ble_free(id: u64) {
    HANDLES.lock().unwrap().remove(&id);
    ENCODERS.lock().unwrap().remove(&id);
}
fn encoder(id: u32, response: bool, limit: u32, data: &[u8]) -> Result<u64, String> {
    ble_wire::frame(data, id, response, 0, limit as usize).map_err(|e| e.to_string())?;
    let mut encoders = ENCODERS.lock().unwrap();
    if encoders.len() >= 32 {
        return Err("BLE encoder capacity exceeded".into());
    }
    let handle = NEXT.fetch_add(1, Ordering::Relaxed);
    encoders.insert(
        handle,
        Encoder {
            data: data.to_vec(),
            id,
            response,
            sequence: 0,
            limit: limit as usize,
        },
    );
    Ok(handle)
}
/// # Safety
/// `data` must reference `len` readable bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plain_ble_encoder(
    id: u32,
    response: bool,
    limit: u32,
    data: *const u8,
    len: usize,
) -> u64 {
    if data.is_null() || len > ble_wire::MAX_MESSAGE {
        return 0;
    }
    encoder(id, response, limit, unsafe {
        std::slice::from_raw_parts(data, len)
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn plain_ble_info(id: u64) -> u64 {
    HANDLES
        .lock()
        .unwrap()
        .get(&id)
        .map(Assembler::info)
        .unwrap_or(0)
}
fn call(
    action: i32,
    handle: u64,
    id: u32,
    sequence: u32,
    limit: u32,
    response: bool,
    data: &[u8],
) -> Result<Option<Vec<u8>>, String> {
    let result = match action {
        0 => ble_wire::message(&[], data).map(Some),
        1 => ble_wire::nearby_body(data).map(|b| Some(b.to_vec())),
        2 => ble_wire::frame(data, id, response, sequence, limit as usize),
        3 => {
            return HANDLES
                .lock()
                .unwrap()
                .get_mut(&handle)
                .ok_or("BLE assembler released".to_string())?
                .push(data)
                .map_err(|e| e.to_string());
        }
        4 => {
            let mut encoders = ENCODERS.lock().unwrap();
            let e = encoders
                .get_mut(&handle)
                .ok_or("BLE encoder released".to_string())?;
            let frame = ble_wire::frame(&e.data, e.id, e.response, e.sequence, e.limit)
                .map_err(|e| e.to_string())?;
            if frame.is_some() {
                e.sequence = e.sequence.checked_add(1).ok_or("BLE sequence overflow")?;
            }
            return Ok(frame);
        }
        9 => {
            if data.len() > 1650 {
                return Err("BLE payload exceeds advertising limit".into());
            }
            let service_uuids: Vec<String> =
                serde_json::from_slice(data).map_err(|e| e.to_string())?;
            serde_json::to_vec(&crate::features::discovery::advertisement::decode_ble(
                &service_uuids,
            ))
            .map(Some)
            .map_err(Into::into)
        }
        _ => return Err("Invalid BLE codec action".into()),
    };
    result.map_err(|e| e.to_string())
}
#[repr(C)]
pub struct Buffer {
    data: *mut u8,
    len: usize,
    status: i32,
}
/// # Safety
/// A non-null `data` must reference `len` readable bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plain_ble_call(
    action: i32,
    handle: u64,
    id: u32,
    sequence: u32,
    limit: u32,
    response: bool,
    data: *const u8,
    len: usize,
) -> Buffer {
    let result = if len > ble_wire::MAX_MESSAGE + 32 || (data.is_null() && len != 0) {
        Err("Invalid BLE input".into())
    } else {
        call(
            action,
            handle,
            id,
            sequence,
            limit,
            response,
            if len == 0 {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(data, len) }
            },
        )
    };
    let (value, status) = match result {
        Ok(Some(v)) => (v, 1),
        Ok(None) => {
            return Buffer {
                data: std::ptr::null_mut(),
                len: 0,
                status: 0,
            };
        }
        Err(e) => (e.into_bytes(), -1),
    };
    let mut value = value.into_boxed_slice();
    let out = Buffer {
        data: value.as_mut_ptr(),
        len: value.len(),
        status,
    };
    std::mem::forget(value);
    out
}
/// # Safety
/// `buffer` must be an unmodified, unfreed result of `plain_ble_call`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plain_ble_buffer_free(buffer: Buffer) {
    if !buffer.data.is_null() {
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) });
    }
}
#[cfg(target_os = "android")]
mod android {
    use super::*;
    use plain_libs::platform::jni_byte_array::{EnvPtr, Object, with_env};
    use std::ptr;

    #[unsafe(no_mangle)]
    unsafe extern "system" fn Java_com_ismartcoding_plain_rust_bridge_RustBleBridge_encoderNative(
        env: EnvPtr,
        _: Object,
        data: Object,
        id: i32,
        response: u8,
        limit: i32,
    ) -> i64 {
        unsafe {
            with_env(env, |env| {
                let Some(bytes) = env.read(data) else {
                    return 0;
                };
                match encoder(id as u32, response != 0, limit as u32, &bytes) {
                    Ok(handle) => handle as i64,
                    Err(error) => {
                        env.illegal_state(&error);
                        0
                    }
                }
            })
        }
    }

    #[unsafe(no_mangle)]
    extern "system" fn Java_com_ismartcoding_plain_rust_bridge_RustBleBridge_newNative(
        _: EnvPtr,
        _: Object,
    ) -> i64 {
        plain_ble_new() as i64
    }

    #[unsafe(no_mangle)]
    extern "system" fn Java_com_ismartcoding_plain_rust_bridge_RustBleBridge_freeNative(
        _: EnvPtr,
        _: Object,
        handle: i64,
    ) {
        plain_ble_free(handle as u64);
    }

    #[unsafe(no_mangle)]
    extern "system" fn Java_com_ismartcoding_plain_rust_bridge_RustBleBridge_infoNative(
        _: EnvPtr,
        _: Object,
        handle: i64,
    ) -> i64 {
        plain_ble_info(handle as u64) as i64
    }

    #[unsafe(no_mangle)]
    unsafe extern "system" fn Java_com_ismartcoding_plain_rust_bridge_RustBleBridge_callNative(
        env: EnvPtr,
        _: Object,
        action: i32,
        handle: i64,
        id: i32,
        sequence: i32,
        limit: i32,
        response: u8,
        data: Object,
    ) -> Object {
        unsafe {
            with_env(env, |env| {
                let Some(data) = env.read(data) else {
                    return ptr::null_mut();
                };
                match call(
                    action,
                    handle as u64,
                    id as u32,
                    sequence as u32,
                    limit as u32,
                    response != 0,
                    &data,
                ) {
                    Ok(Some(bytes)) => env.write(&bytes),
                    Ok(None) => ptr::null_mut(),
                    Err(error) => {
                        env.illegal_state(&error);
                        ptr::null_mut()
                    }
                }
            })
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/ffi/ble.rs"]
mod tests;
