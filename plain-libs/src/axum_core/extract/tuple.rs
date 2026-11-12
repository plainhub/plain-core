use super::{FromRequest, FromRequestParts, Request};
use crate::axum_core::response::{IntoResponse, Response};
use crate::http::request::Parts;
use std::convert::Infallible;

impl<S> FromRequestParts<S> for ()
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(_: &'extract mut Parts, _: &'extract S) -> Result<(), Self::Rejection>  where Self: 'extract {
        Ok(())
    }
}
macro_rules! impl_from_request { ([$($ty : ident : $var : ident) ,*] , $last : ident : $last_var : ident) => {  impl < S , $($ty ,) * $last > FromRequestParts < S > for ($($ty ,) * $last ,) where $($ty : FromRequestParts < S > + Send ,) * $last : FromRequestParts < S > + Send , S : Send + Sync , { type Rejection = Response ; async fn from_request_parts<'extract>(parts : &'extract  mut Parts , state : &'extract  S) -> Result < Self , Self::Rejection >  where Self: 'extract { $(let $var = $ty::from_request_parts (parts , state) . await . map_err (| err | err . into_response ()) ?;) * let $last_var = $last::from_request_parts (parts , state) . await . map_err (| err | err . into_response ()) ?; Ok (($($var ,) * $last_var ,)) } }  impl < S , $($ty ,) * $last > FromRequest < S > for ($($ty ,) * $last ,) where $($ty : FromRequestParts < S > + Send ,) * $last : FromRequest < S > + Send , S : Send + Sync , { type Rejection = Response ; async fn from_request<'extract>(req : Request , state : &'extract  S) -> Result < Self , Self::Rejection >  where Self: 'extract { let (mut parts , body) = req . into_parts () ; $(let $var = $ty::from_request_parts (& mut parts , state) . await . map_err (| err | err . into_response ()) ?;) * let req = Request::from_parts (parts , body) ; let $last_var = $last::from_request (req , state) . await . map_err (| err | err . into_response ()) ?; Ok (($($var ,) * $last_var ,)) } } } ; }
for_http_tuples!(last, impl_from_request);
