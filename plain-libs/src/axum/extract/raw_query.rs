use super::FromRequestParts;
use crate::http::request::Parts;
use std::convert::Infallible;
#[derive(Debug)]
pub struct RawQuery(pub Option<String>);

impl<S> FromRequestParts<S> for RawQuery
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let query = parts.uri.query().map(|query| query.to_owned());
        Ok(Self(query))
    }
}
