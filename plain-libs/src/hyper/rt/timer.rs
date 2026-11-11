use std::{
    any::TypeId,
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};
pub trait Timer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>>;
    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>>;
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn reset(&self, sleep: &mut Pin<Box<dyn Sleep>>, new_deadline: Instant) {
        *sleep = self.sleep_until(new_deadline);
    }
}
pub trait Sleep: Send + Sync + Future<Output = ()> {
    fn __type_id(&self, _: private::Sealed) -> TypeId
    where
        Self: 'static,
    {
        TypeId::of::<Self>()
    }
}
impl dyn Sleep {
    pub fn is<T>(&self) -> bool
    where
        T: Sleep + 'static,
    {
        self.__type_id(private::Sealed) == TypeId::of::<T>()
    }
    pub fn downcast_mut_pin<T>(self: Pin<&mut Self>) -> Option<Pin<&mut T>>
    where
        T: Sleep + 'static,
    {
        if self.is::<T>() {
            unsafe {
                let inner = Pin::into_inner_unchecked(self);
                Some(Pin::new_unchecked(
                    &mut *(&mut *inner as *mut dyn Sleep).cast(),
                ))
            }
        } else {
            None
        }
    }
}
mod private {
    pub struct Sealed;
}
