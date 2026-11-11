#![forbid(unsafe_code)]
#[macro_use]
pub(crate) mod macros;

pub mod layer;

pub mod util;
pub use self::util::{ServiceExt, service_fn};

pub use tower_layer::Layer;
pub use tower_service::Service;
mod sealed {
    pub trait Sealed<T> {}
}
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
