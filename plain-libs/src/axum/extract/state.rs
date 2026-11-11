use crate::axum_core::extract::{FromRef, FromRequestParts};
use crate::http::request::Parts;
use async_trait::async_trait;
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};
#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);
#[async_trait]
impl<OuterState, InnerState> FromRequestParts<OuterState> for State<InnerState>
where
    InnerState: FromRef<OuterState>,
    OuterState: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts(
        _parts: &mut Parts,
        state: &OuterState,
    ) -> Result<Self, Self::Rejection> {
        let inner_state = InnerState::from_ref(state);
        Ok(Self(inner_state))
    }
}
impl<S> Deref for State<S> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<S> DerefMut for State<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
