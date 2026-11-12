#![forbid(unsafe_code)]
#[macro_use]
pub(crate) mod macros;

pub mod layer;
mod service;

pub mod util;
pub use self::util::{ServiceExt, service_fn};

pub use self::layer::Layer;
pub use self::service::Service;
mod sealed {
    pub trait Sealed<T> {}
}
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
