pub use crate::http;
pub use crate::http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version, header};
pub use crate::hyper::error::{Error, Result};
#[macro_use]
mod cfg;
#[macro_use]
mod trace;
pub mod body;
mod common;
mod error;
pub mod ext;
pub mod rt;
pub mod service;
pub mod upgrade;
cfg_proto! { mod headers ; mod proto ; }
cfg_feature! { #! [all ()] pub mod client ; }
cfg_feature! { #! [all ()] pub mod server ; }

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/hyper.rs"]
mod tests;
