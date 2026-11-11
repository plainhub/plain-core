#[macro_use]
mod macros;
mod alloc;
mod control;
mod hasher;
mod raw;
mod scopeguard;
mod table;
mod util;
pub use crate::hashbrown::hasher::DefaultHashBuilder;
pub use crate::hashbrown::hasher::DefaultHasher;
pub mod hash_table {
    pub use crate::hashbrown::table::*;
}
pub use crate::equivalent::Equivalent;
pub use crate::hashbrown::table::HashTable;
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TryReserveError {
    CapacityOverflow,
    AllocError { layout: std::alloc::Layout },
}
impl core::fmt::Display for TryReserveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("memory allocation failed")?;
        let reason = match self {
            TryReserveError::CapacityOverflow => {
                " because the computed capacity exceeded the collection's maximum"
            }
            TryReserveError::AllocError { .. } => " because the memory allocator returned an error",
        };
        f.write_str(reason)
    }
}
impl core::error::Error for TryReserveError {}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/hashbrown.rs"]
mod tests;
