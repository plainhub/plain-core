use super::{
    FromRequestParts,
    rejection::{FailedToResolveHost, HostRejection},
};
use crate::http::{
    header::{FORWARDED, HeaderMap},
    request::Parts,
};
use async_trait::async_trait;
const X_FORWARDED_HOST_HEADER_KEY: &str = "X-Forwarded-Host";
#[derive(Debug, Clone)]
pub struct Host(pub String);
#[async_trait]
impl<S> FromRequestParts<S> for Host
where
    S: Send + Sync,
{
    type Rejection = HostRejection;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(host) = parse_forwarded(&parts.headers) {
            return Ok(Host(host.to_owned()));
        }
        if let Some(host) = parts
            .headers
            .get(X_FORWARDED_HOST_HEADER_KEY)
            .and_then(|host| host.to_str().ok())
        {
            return Ok(Host(host.to_owned()));
        }
        if let Some(host) = parts
            .headers
            .get(crate::http::header::HOST)
            .and_then(|host| host.to_str().ok())
        {
            return Ok(Host(host.to_owned()));
        }
        if let Some(host) = parts.uri.host() {
            return Ok(Host(host.to_owned()));
        }
        Err(HostRejection::FailedToResolveHost(FailedToResolveHost))
    }
}
fn parse_forwarded(headers: &HeaderMap) -> Option<&str> {
    let forwarded_values = headers.get(FORWARDED)?.to_str().ok()?;
    let first_value = forwarded_values.split(',').nth(0)?;
    first_value.split(';').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("host")
            .then(|| value.trim().trim_matches('"'))
    })
}
