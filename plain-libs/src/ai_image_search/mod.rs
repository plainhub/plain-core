pub mod embedding;
pub mod models;
mod preprocess;
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

#[cfg(feature = "image-search-inference")]
use tract::Session;

#[cfg(not(feature = "image-search-inference"))]
use unavailable::Session;
#[cfg(not(feature = "image-search-inference"))]
mod unavailable {
    use super::Input;
    use std::path::Path;

    pub enum Session {}
    impl Session {
        pub fn new(_: &Path, _: &[Input<'_>], _: &str) -> Result<Self, String> {
            Err("AI image search is not available in this build".into())
        }

        pub fn run(&mut self, _: &mut [Input<'_>], _: usize) -> Result<Vec<f32>, String> {
            match *self {}
        }
    }
}
