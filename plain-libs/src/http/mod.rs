#[macro_use]
mod convert;
mod byte_str;
mod error;
mod extensions;
pub mod header;
pub mod method;
pub mod request;
pub mod response;
pub mod status;
pub mod uri;
pub mod version;
pub use crate::http::error::{Error, Result};
pub use crate::http::extensions::Extensions;
pub use crate::http::header::{HeaderMap, HeaderName, HeaderValue};
pub use crate::http::method::Method;
pub use crate::http::request::Request;
pub use crate::http::response::Response;
pub use crate::http::status::StatusCode;
pub use crate::http::uri::Uri;
pub use crate::http::version::Version;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/http.rs"]
mod tests;
