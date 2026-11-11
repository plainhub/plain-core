use crate::hyper::rt::Sleep;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
#[derive(Clone)]
pub(crate) struct Timer(Arc<dyn crate::hyper::rt::Timer + Send + Sync>);
impl Timer {
    pub(crate) fn new<T>(inner: T) -> Self
    where
        T: crate::hyper::rt::Timer + Send + Sync + 'static,
    {
        Self(Arc::new(inner))
    }
}
impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Timer").finish()
    }
}
impl crate::hyper::rt::Timer for Timer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
        self.0.sleep(duration)
    }
    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
        self.0.sleep_until(deadline)
    }
    fn now(&self) -> Instant {
        self.0.now()
    }
}
