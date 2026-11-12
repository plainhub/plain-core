use super::{
    has_content_type,
    rejection::{InvalidFormContentType, RawFormRejection},
};
use crate::axum_core::extract::{FromRequest, Request};
use crate::http::Method;
use bytes::Bytes;
#[derive(Debug)]
pub struct RawForm(pub Bytes);

impl<S> FromRequest<S> for RawForm
where
    S: Send + Sync,
{
    type Rejection = RawFormRejection;
    async fn from_request<'extract>(req: Request, state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        if req.method() == Method::GET {
            if let Some(query) = req.uri().query() {
                return Ok(Self(Bytes::copy_from_slice(query.as_bytes())));
            }
            Ok(Self(Bytes::new()))
        } else {
            if !has_content_type(req.headers(), &crate::mime::APPLICATION_WWW_FORM_URLENCODED) {
                return Err(InvalidFormContentType.into());
            }
            Ok(Self(Bytes::from_request(req, state).await?))
        }
    }
}
