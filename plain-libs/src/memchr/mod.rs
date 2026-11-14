#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
compile_error!("memchr currently not supported on non-{16,32,64}");
pub use crate::memchr::memchr::{
    Memchr, Memchr2, Memchr3, memchr, memchr_iter, memchr2, memchr2_iter, memchr3, memchr3_iter,
    memrchr, memrchr_iter, memrchr2, memrchr2_iter, memrchr3, memrchr3_iter,
};
#[macro_use]
mod macros;
pub mod arch;
mod cow;
pub(crate) mod ext;
mod memchr;
pub mod memmem;
pub(crate) mod vector;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/memchr.rs"]
mod tests;
