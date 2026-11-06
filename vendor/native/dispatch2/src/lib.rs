#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
#[macro_use]
mod macros;
mod generated;
mod object;
mod queue;
mod retained;
use core::{
    cell::UnsafeCell,
    marker::{PhantomData, PhantomPinned},
};
pub(crate) use object::dispatch_object_s;
pub use object::DispatchObject;
pub use queue::{DispatchQueue, DispatchQueueAttr};
pub use retained::DispatchRetained;
type OpaqueData = UnsafeCell<PhantomData<(*const UnsafeCell<()>, PhantomPinned)>>;
#[cfg_attr(target_vendor = "apple", link(name = "System", kind = "dylib"))]
extern "C" {}
