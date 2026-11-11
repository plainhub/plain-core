use super::FromRequestParts;
use crate::http::request::Parts;
use async_trait::async_trait;
use std::convert::Infallible;
#[derive(Debug)]
pub struct RawQuery(pub Option<String>);
#[async_trait]
impl<S> FromRequestParts<S> for RawQuery
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().map(|query| query.to_owned());
        Ok(Self(query))
    }
}
