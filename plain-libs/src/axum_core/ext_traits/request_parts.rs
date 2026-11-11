use crate::axum_core::extract::FromRequestParts;
use crate::http::request::Parts;
use futures_util::future::BoxFuture;
mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::http::request::Parts {}
}
pub trait RequestPartsExt: sealed::Sealed + Sized {
    fn extract<E>(&mut self) -> BoxFuture<'_, Result<E, E::Rejection>>
    where
        E: FromRequestParts<()> + 'static;
    fn extract_with_state<'a, E, S>(
        &'a mut self,
        state: &'a S,
    ) -> BoxFuture<'a, Result<E, E::Rejection>>
    where
        E: FromRequestParts<S> + 'static,
        S: Send + Sync;
}
impl RequestPartsExt for Parts {
    fn extract<E>(&mut self) -> BoxFuture<'_, Result<E, E::Rejection>>
    where
        E: FromRequestParts<()> + 'static,
    {
        self.extract_with_state(&())
    }
    fn extract_with_state<'a, E, S>(
        &'a mut self,
        state: &'a S,
    ) -> BoxFuture<'a, Result<E, E::Rejection>>
    where
        E: FromRequestParts<S> + 'static,
        S: Send + Sync,
    {
        E::from_request_parts(self, state)
    }
}
