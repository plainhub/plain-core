use crate::{
    ContextSelectionSet, OutputType, Positioned, ServerResult, Value, parser::types::Field,
};
pub async fn resolve_list<'a, T: OutputType + 'a>(
    ctx: &ContextSelectionSet<'a>,
    field: &Positioned<Field>,
    iter: impl IntoIterator<Item = T>,
    len: Option<usize>,
) -> ServerResult<Value> {
    let mut futures = len.map(Vec::with_capacity).unwrap_or_default();
    for (idx, item) in iter.into_iter().enumerate() {
        let ctx_idx = ctx.with_index(idx);
        futures.push(async move {
            OutputType::resolve(&item, &ctx_idx, field)
                .await
                .map_err(|err| ctx_idx.set_error_path(err))
        });
    }
    Ok(Value::List(
        futures_util::future::try_join_all(futures).await?,
    ))
}
