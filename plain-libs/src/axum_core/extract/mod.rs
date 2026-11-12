use crate::axum_core::{body::Body, response::IntoResponse};
use crate::http::request::Parts;
use std::convert::Infallible;
mod default_body_limit;
mod from_ref;
pub mod rejection;
mod request_parts;
mod tuple;
pub(crate) use self::default_body_limit::DefaultBodyLimitKind;
pub use self::{default_body_limit::DefaultBodyLimit, from_ref::FromRef};
pub type Request<T = Body> = crate::http::Request<T>;
mod private {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}
    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}


pub trait FromRequestParts<S>: Sized {
    type Rejection: IntoResponse;
    fn from_request_parts<'extract>(parts: &'extract mut Parts, state: &'extract S) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'extract where Self: 'extract;
}


pub trait FromRequest<S, M = private::ViaRequest>: Sized {
    type Rejection: IntoResponse;
    fn from_request<'extract>(req: Request, state: &'extract S) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'extract where Self: 'extract;
}

impl<S, T> FromRequest<S, private::ViaParts> for T
where
    S: Send + Sync,
    T: FromRequestParts<S>,
{
    type Rejection = <Self as FromRequestParts<S>>::Rejection;
    async fn from_request<'extract>(req: Request, state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let (mut parts, _) = req.into_parts();
        Self::from_request_parts(&mut parts, state).await
    }
}

impl<S, T> FromRequestParts<S> for Option<T>
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(
        parts: &'extract mut Parts,
        state: &'extract S,
    ) -> Result<Option<T>, Self::Rejection>  where Self: 'extract {
        Ok(T::from_request_parts(parts, state).await.ok())
    }
}

impl<S, T> FromRequest<S> for Option<T>
where
    T: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request<'extract>(req: Request, state: &'extract S) -> Result<Option<T>, Self::Rejection>  where Self: 'extract {
        Ok(T::from_request(req, state).await.ok())
    }
}

impl<S, T> FromRequestParts<S> for Result<T, T::Rejection>
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(T::from_request_parts(parts, state).await)
    }
}

impl<S, T> FromRequest<S> for Result<T, T::Rejection>
where
    T: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request<'extract>(req: Request, state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        Ok(T::from_request(req, state).await)
    }
}
