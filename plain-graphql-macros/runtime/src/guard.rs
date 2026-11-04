use crate::{Context, Result};
use std::future::Future;
pub trait Guard {
    fn check(&self, ctx: &Context<'_>) -> impl Future<Output = Result<()>> + Send;
}
