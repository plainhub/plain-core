#![forbid(unsafe_code)]
pub extern crate core as _core;
#[path = "bool.rs"]
pub mod _bool;
pub use _bool::{False, True};
mod assert_impl;
mod does_impl;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/assertions/lib.rs"]
mod tests;

pub use crate::{_does_impl, assert_impl};
