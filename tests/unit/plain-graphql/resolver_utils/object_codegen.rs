use std::future::Future;

pub type Value = u64;
pub type ServerResult<T> = Result<T, Error>;
pub struct Error;
impl Error {
    fn into_server_error(self, _: u8) -> Self {
        self
    }
}
pub struct Node {
    selection_set: u8,
}
pub struct Field {
    pos: u8,
    node: Node,
}
pub struct Context<'a> {
    item: &'a Field,
}
pub struct ContextSelectionSet<'a> {
    _item: &'a Field,
}
impl Context<'_> {
    fn set_error_path(&self, error: Error) -> Error {
        error
    }
    fn with_selection_set(&self, _: &u8) -> ContextSelectionSet<'_> {
        ContextSelectionSet { _item: self.item }
    }
}
pub trait OutputType: Send + Sync {
    fn resolve<'a>(
        &'a self,
        _: &'a ContextSelectionSet<'a>,
        _: &'a Field,
    ) -> impl Future<Output = ServerResult<Value>> + Send + 'a;
}
struct Number(u64);
impl OutputType for Number {
    async fn resolve<'a>(
        &'a self,
        _: &'a ContextSelectionSet<'a>,
        _: &'a Field,
    ) -> ServerResult<Value> {
        Ok(std::hint::black_box(self.0))
    }
}
#[path = "RESOLVER_SOURCE"]
mod subject;

#[unsafe(no_mangle)]
pub fn probe(value: u64) -> u64 {
    let field = Field {
        pos: 0,
        node: Node { selection_set: 0 },
    };
    let ctx = Context { item: &field };
    let future =
        subject::resolve_field_async(&ctx, std::future::ready(Ok::<Number, Error>(Number(value))));
    let mut future = std::pin::pin!(future);
    let mut task = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut task) {
        std::task::Poll::Ready(Ok(Some(value))) => value,
        _ => 0,
    }
}
