use crate::hyper::rt::Executor;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
pub(crate) type BoxSendFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
#[derive(Clone)]
pub(crate) enum Exec {
    Executor(Arc<dyn Executor<BoxSendFuture> + Send + Sync>),
}
impl Exec {
    pub(crate) fn new<E>(inner: E) -> Self
    where
        E: Executor<BoxSendFuture> + Send + Sync + 'static,
    {
        Exec::Executor(Arc::new(inner))
    }
    pub(crate) fn execute<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        match *self {
            Exec::Executor(ref e) => {
                e.execute(Box::pin(fut));
            }
        }
    }
}
impl fmt::Debug for Exec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exec").finish()
    }
}
impl<F> crate::hyper::rt::Executor<F> for Exec
where
    F: Future<Output = ()> + Send + 'static,
{
    fn execute(&self, fut: F) {
        Exec::execute(self, fut);
    }
}
