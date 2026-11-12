use crate::axum_core::extract::{FromRef, FromRequestParts};
use crate::http::request::Parts;
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};
#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);

impl<OuterState, InnerState> FromRequestParts<OuterState> for State<InnerState>
where
    InnerState: FromRef<OuterState>,
    OuterState: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts<'extract>(
        _parts: &'extract mut Parts,
        state: &'extract OuterState,
    ) -> Result<Self, Self::Rejection>  where Self: 'extract {
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
