use crate::http::header::{self, HeaderMap};
pub mod connect_info;
mod host;
pub(crate) mod nested_path;
pub mod path;
mod raw_form;
mod raw_query;
pub mod rejection;

mod state;
pub use self::connect_info::ConnectInfo;
pub use self::{
    host::Host,
    nested_path::NestedPath,
    path::{Path, RawPathParams},
    raw_form::RawForm,
    raw_query::RawQuery,
    state::State,
};
pub use crate::axum::Extension;
pub use crate::axum::Json;
pub use crate::axum_core::extract::{
    DefaultBodyLimit, FromRef, FromRequest, FromRequestParts, Request,
};
mod query;
pub use self::query::Query;
pub(super) fn has_content_type(
    headers: &HeaderMap,
    expected_content_type: &crate::mime::Mime,
) -> bool {
    let content_type = if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };
    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };
    content_type.starts_with(expected_content_type.as_ref())
}
