use crate::{Context, Error, OutputType, ServerResult, Value};
use std::future::Future;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "../../../tests/unit/plain-graphql/resolver_utils/object.rs"]
mod tests;

#[doc(hidden)]
pub fn resolve_field_async<'a, T, E, F>(
    ctx: &'a Context<'a>,
    fut: F,
) -> impl Future<Output = ServerResult<Option<Value>>> + Send + 'a
where
    T: OutputType + Send,
    E: Into<Error> + Send + Sync,
    F: Future<Output = Result<T, E>> + Send + 'a,
{
    // Keep the polling body out of generated resolver callers to limit LLVM code size.
    #[inline(never)]
    async move {
        let obj: T = fut.await.map_err(|err| {
            let err = ::std::convert::Into::<Error>::into(err).into_server_error(ctx.item.pos);
            ctx.set_error_path(err)
        })?;
        let ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
        OutputType::resolve(&obj, &ctx_obj, ctx.item)
            .await
            .map(Option::Some)
    }
}
#[doc(hidden)]
#[inline(never)]
pub async fn resolve_simple_field_value<T: OutputType + ?Sized>(
    ctx: &Context<'_>,
    value: &T,
) -> ServerResult<Option<Value>> {
    OutputType::resolve(
        value,
        &ctx.with_selection_set(&ctx.item.node.selection_set),
        ctx.item,
    )
    .await
    .map(Option::Some)
    .map_err(|err| ctx.set_error_path(err))
}
