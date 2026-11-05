#![no_std]
pub trait EnumCount {
    const COUNT: usize;
}

#[cfg(feature = "derive")]
pub use strum_macros::{EnumCount, FromRepr};

#[cfg(all(test, feature = "derive"))]
#[path = "../tests/unit/src/lib.rs"]
mod tests;
