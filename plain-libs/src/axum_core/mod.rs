#![forbid(unsafe_code)]
#[macro_use]
pub(crate) mod macros;
pub mod __private {}
mod error;
mod ext_traits;
pub use self::error::Error;
pub mod body;
pub mod extract;
pub mod response;
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub use self::ext_traits::request::RequestExt;
pub use crate::{
    plain_axum_core___composite_rejection as __composite_rejection,
    plain_axum_core___define_rejection as __define_rejection,
    plain_axum_core___impl_deref as __impl_deref,
    plain_axum_core___log_rejection as __log_rejection,
};
