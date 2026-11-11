use super::{FromRequestParts, rejection::*};
use crate::http::{Uri, request::Parts};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);
#[async_trait]
impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryRejection;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Self::try_from_uri(&parts.uri)
    }
}
impl<T> Query<T>
where
    T: DeserializeOwned,
{
    pub fn try_from_uri(value: &Uri) -> Result<Self, QueryRejection> {
        let query = value.query().unwrap_or_default();
        let params = crate::serde_urlencoded::from_str(query)
            .map_err(FailedToDeserializeQueryString::from_err)?;
        Ok(Query(params))
    }
}
crate::axum_core::__impl_deref!(Query);
