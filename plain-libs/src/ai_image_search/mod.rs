pub mod embedding;
pub mod models;
mod preprocess;
#[cfg(feature = "onnx-runtime")]
mod session;

#[repr(C)]
struct Input {
    name: *const std::ffi::c_char,
    data: *mut std::ffi::c_void,
    bytes: usize,
    shape: *const i64,
    rank: usize,
    element_type: i32,
}

#[cfg(not(feature = "onnx-runtime"))]
mod session {
    use super::Input;
    use std::{path::Path, sync::Arc};

    pub enum Context {}
    impl Context {
        pub fn new() -> Result<Arc<Self>, String> {
            Err("AI image search is not available in this build".into())
        }
    }

    pub enum Session {}
    impl Session {
        pub fn new(context: &Arc<Context>, _: &Path) -> Result<Self, String> {
            match **context {}
        }

        pub fn run(&mut self, _: &mut [Input], _: &str, _: usize) -> Result<Vec<f32>, String> {
            match *self {}
        }
    }
}
