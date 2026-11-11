pub use crate::axum_core::body::{Body, BodyDataStream};
pub use crate::http_body::Body as HttpBody;
use crate::http_body_util::{BodyExt, Limited};
pub use bytes::Bytes;
pub async fn to_bytes(body: Body, limit: usize) -> Result<Bytes, crate::axum_core::Error> {
    Limited::new(body, limit)
        .collect()
        .await
        .map(|col| col.to_bytes())
        .map_err(crate::axum_core::Error::new)
}
