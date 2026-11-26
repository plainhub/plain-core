use super::{Input, Values};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    ptr::NonNull,
    sync::{Arc, Mutex},
};
#[repr(C)]
struct OrtInput {
    name: *const c_char,
    data: *mut c_void,
    bytes: usize,
    shape: *const i64,
    rank: usize,
    element_type: i32,
}

unsafe extern "C" {
    fn plain_onnx_context(
        library: *const c_char,
        error: *mut c_char,
        capacity: usize,
    ) -> *mut c_void;
    fn plain_onnx_close_context(context: *mut c_void);
    fn plain_onnx_create(
        context: *mut c_void,
        model: *const c_char,
        threads: i32,
        provider: *const c_char,
        error: *mut c_char,
        capacity: usize,
    ) -> *mut c_void;
    fn plain_onnx_close(session: *mut c_void);
    fn plain_onnx_run(
        session: *mut c_void,
        inputs: *const OrtInput,
        count: usize,
        output_name: *const c_char,
        output: *mut f32,
        dimensions: usize,
        error: *mut c_char,
        capacity: usize,
    ) -> i32;
}
static CONTEXT_LIFECYCLE: Mutex<()> = Mutex::new(());

pub struct NativeContext(NonNull<c_void>);
unsafe impl Send for NativeContext {}
unsafe impl Sync for NativeContext {}
pub type Context = Arc<NativeContext>;

pub fn create_context() -> Result<Context, String> {
    #[cfg(target_os = "android")]
    let library = CString::new("libonnxruntime.so").unwrap();
    #[cfg(target_os = "ios")]
    let library = CString::new("linked ONNX Runtime").unwrap();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let library = CString::new(library_path()).map_err(|e| e.to_string())?;
    let mut error = [0 as c_char; 2048];
    let _lifecycle = CONTEXT_LIFECYCLE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let pointer = unsafe { plain_onnx_context(library.as_ptr(), error.as_mut_ptr(), error.len()) };
    NonNull::new(pointer)
        .map(|pointer| Arc::new(NativeContext(pointer)))
        .ok_or_else(|| {
            unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })
}
impl Drop for NativeContext {
    fn drop(&mut self) {
        let _lifecycle = CONTEXT_LIFECYCLE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe {
            plain_onnx_close_context(self.0.as_ptr());
        }
    }
}
pub struct Session {
    pointer: NonNull<c_void>,
    _context: Context,
}
unsafe impl Send for Session {}
impl Session {
    pub fn new(
        context: &Context,
        model: &std::path::Path,
        _: &[Input<'_>],
        _: &str,
    ) -> Result<Self, String> {
        let mut error = [0 as c_char; 2048];
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4)) as i32;
        #[cfg(unix)]
        let model = CString::new(std::os::unix::ffi::OsStrExt::as_bytes(model.as_os_str()))
            .map_err(|e| e.to_string())?;
        #[cfg(not(unix))]
        let model = CString::new(model.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let provider =
            CString::new(std::env::var("PLAIN_ONNX_PROVIDER").unwrap_or_else(|_| "CPU".into()))
                .map_err(|e| e.to_string())?;
        let mut pointer = unsafe {
            plain_onnx_create(
                context.0.as_ptr(),
                model.as_ptr(),
                threads,
                provider.as_ptr(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if pointer.is_null() && provider.to_bytes() != b"CPU" {
            log::warn!(
                "ONNX provider initialization failed: {}",
                unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
            );
            pointer = unsafe {
                plain_onnx_create(
                    context.0.as_ptr(),
                    model.as_ptr(),
                    threads,
                    c"CPU".as_ptr(),
                    error.as_mut_ptr(),
                    error.len(),
                )
            };
        }
        NonNull::new(pointer)
            .map(|pointer| Self {
                pointer,
                _context: Arc::clone(context),
            })
            .ok_or_else(|| {
                unsafe { CStr::from_ptr(error.as_ptr()) }
                    .to_string_lossy()
                    .into_owned()
            })
    }
    pub fn run(
        &mut self,
        inputs: &mut [Input<'_>],
        name: &str,
        dimensions: usize,
    ) -> Result<Vec<f32>, String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let names = inputs
            .iter()
            .map(|input| CString::new(input.name))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let inputs = inputs
            .iter_mut()
            .zip(&names)
            .map(|(input, name)| {
                let (data, bytes, element_type) = match &mut input.values {
                    Values::Float(values) => (
                        values.as_mut_ptr().cast(),
                        std::mem::size_of_val(values.as_slice()),
                        1,
                    ),
                    Values::Integer(values) => (
                        values.as_mut_ptr().cast(),
                        std::mem::size_of_val(values.as_slice()),
                        7,
                    ),
                };
                OrtInput {
                    name: name.as_ptr(),
                    data,
                    bytes,
                    shape: input.shape.as_ptr(),
                    rank: input.shape.len(),
                    element_type,
                }
            })
            .collect::<Vec<_>>();
        let mut values = vec![0.0; dimensions];
        let mut error = [0 as c_char; 2048];
        let code = unsafe {
            plain_onnx_run(
                self.pointer.as_ptr(),
                inputs.as_ptr(),
                inputs.len(),
                name.as_ptr(),
                values.as_mut_ptr(),
                values.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if code != 0 {
            return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        crate::ai_image_search::embedding::normalize(&mut values)?;
        Ok(values)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            plain_onnx_close(self.pointer.as_ptr());
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn library_path() -> String {
    if let Ok(path) = std::env::var("PLAIN_ONNX_RUNTIME_LIBRARY") {
        return path;
    }
    let name = if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            if let Some(path) = packaged_library(directory, name) {
                return path.to_string_lossy().into_owned();
            }
        }
    }
    let development = std::path::Path::new(env!("PLAIN_ONNX_RUNTIME_LIBRARY"));
    if development.is_file() {
        development.to_string_lossy().into_owned()
    } else {
        name.into()
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn packaged_library(directory: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    [
        directory.join(name),
        directory.join("../Resources").join(name),
    ]
    .into_iter()
    .chain(cfg!(target_os = "macos").then(|| directory.join("../Frameworks").join(name)))
    .find(|path| path.is_file())
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/ai_image_search/session.rs"]
mod tests;
