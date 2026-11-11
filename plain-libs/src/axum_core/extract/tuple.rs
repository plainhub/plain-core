use super::{FromRequest, FromRequestParts, Request};
use crate::axum_core::response::{IntoResponse, Response};
use crate::http::request::Parts;
use async_trait::async_trait;
use std::convert::Infallible;
#[async_trait]
impl<S> FromRequestParts<S> for ()
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts(_: &mut Parts, _: &S) -> Result<(), Self::Rejection> {
        Ok(())
    }
}
macro_rules! impl_from_request { ([$($ty : ident : $var : ident) ,*] , $last : ident : $last_var : ident) => { #[async_trait] impl < S , $($ty ,) * $last > FromRequestParts < S > for ($($ty ,) * $last ,) where $($ty : FromRequestParts < S > + Send ,) * $last : FromRequestParts < S > + Send , S : Send + Sync , { type Rejection = Response ; async fn from_request_parts (parts : & mut Parts , state : & S) -> Result < Self , Self::Rejection > { $(let $var = $ty::from_request_parts (parts , state) . await . map_err (| err | err . into_response ()) ?;) * let $last_var = $last::from_request_parts (parts , state) . await . map_err (| err | err . into_response ()) ?; Ok (($($var ,) * $last_var ,)) } } #[async_trait] impl < S , $($ty ,) * $last > FromRequest < S > for ($($ty ,) * $last ,) where $($ty : FromRequestParts < S > + Send ,) * $last : FromRequest < S > + Send , S : Send + Sync , { type Rejection = Response ; async fn from_request (req : Request , state : & S) -> Result < Self , Self::Rejection > { let (mut parts , body) = req . into_parts () ; $(let $var = $ty::from_request_parts (& mut parts , state) . await . map_err (| err | err . into_response ()) ?;) * let req = Request::from_parts (parts , body) ; let $last_var = $last::from_request (req , state) . await . map_err (| err | err . into_response ()) ?; Ok (($($var ,) * $last_var ,)) } } } ; }
for_http_tuples!(last, impl_from_request);
