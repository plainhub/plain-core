use super::{FromRequest, FromRequestParts, Request, rejection::*};
use crate::axum_core::{RequestExt, body::Body};
use crate::http::{Extensions, HeaderMap, Method, Uri, Version, request::Parts};
use crate::http_body_util::BodyExt;
use bytes::{BufMut, Bytes, BytesMut};
use std::convert::Infallible;

impl<S> FromRequest<S> for Request
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request<'extract>(req: Request, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(req)
    }
}

impl<S> FromRequestParts<S> for Method
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(parts.method.clone())
    }
}

impl<S> FromRequestParts<S> for Uri
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(parts.uri.clone())
    }
}

impl<S> FromRequestParts<S> for Version
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(parts.version)
    }
}

impl<S> FromRequestParts<S> for HeaderMap
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(parts.headers.clone())
    }
}

impl<S> FromRequest<S> for BytesMut
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;
    async fn from_request<'extract>(req: Request, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let mut body = req.into_limited_body();
        let mut bytes = BytesMut::new();
        body_to_bytes_mut(&mut body, &mut bytes).await?;
        Ok(bytes)
    }
}
async fn body_to_bytes_mut(body: &mut Body, bytes: &mut BytesMut) -> Result<(), BytesRejection> {
    while let Some(frame) = body
        .frame()
        .await
        .transpose()
        .map_err(FailedToBufferBody::from_err)?
    {
        let Ok(data) = frame.into_data() else {
            return Ok(());
        };
        bytes.put(data);
    }
    Ok(())
}

impl<S> FromRequest<S> for Bytes
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;
    async fn from_request<'extract>(req: Request, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let bytes = req
            .into_limited_body()
            .collect()
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_bytes();
        Ok(bytes)
    }
}

impl<S> FromRequest<S> for String
where
    S: Send + Sync,
{
    type Rejection = StringRejection;
    async fn from_request<'extract>(req: Request, state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|err| match err {
                BytesRejection::FailedToBufferBody(inner) => {
                    StringRejection::FailedToBufferBody(inner)
                }
            })?;
        let string = String::from_utf8(bytes.into()).map_err(InvalidUtf8::from_err)?;
        Ok(string)
    }
}

impl<S> FromRequestParts<S> for Parts
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(parts.clone())
    }
}

impl<S> FromRequestParts<S> for Extensions
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(parts.extensions.clone())
    }
}

impl<S> FromRequest<S> for Body
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request<'extract>(req: Request, _: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(req.into_body())
    }
}
