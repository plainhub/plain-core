#![forbid(unsafe_code)]
#[macro_use]
pub(crate) mod macros;
pub mod body;
mod boxed;
pub mod error_handling;
mod extension;
pub mod extract;
pub mod handler;
mod json;
pub mod middleware;
pub mod response;
pub mod routing;
pub mod serve;
mod service_ext;
mod util;
pub use self::extension::Extension;
pub use self::json::Json;
pub use self::routing::Router;
pub use self::serve::serve;
pub use self::service_ext::ServiceExt;
pub use crate::axum_core::{BoxError, Error, RequestExt, RequestPartsExt};
pub use crate::http;
pub use async_trait::async_trait;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/axum.rs"]
mod tests;
