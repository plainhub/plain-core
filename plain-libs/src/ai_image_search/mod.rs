pub mod embedding;
pub mod models;
mod preprocess;
#[cfg(feature = "onnx-runtime")]
mod session;
#[cfg(feature = "image-search-inference")]
mod tract;

struct Input<'a> {
    name: &'a str,
    shape: &'a [i64],
    values: Values,
}

enum Values {
    Float(Vec<f32>),
    Integer(Vec<i64>),
}

#[cfg(all(feature = "image-search-inference", not(feature = "onnx-runtime")))]
use tract as session;

#[cfg(not(any(feature = "image-search-inference", feature = "onnx-runtime")))]
mod session {
    use super::Input;
    use std::path::Path;

    pub enum Context {}
    pub fn create_context() -> Result<Context, String> {
        Err("AI image search is not available in this build".into())
    }

    pub enum Session {}
    impl Session {
        pub fn new(context: &Context, _: &Path, _: &[Input<'_>], _: &str) -> Result<Self, String> {
            match *context {}
        }

        pub fn run(&mut self, _: &mut [Input<'_>], _: &str, _: usize) -> Result<Vec<f32>, String> {
            match *self {}
        }
    }
}
