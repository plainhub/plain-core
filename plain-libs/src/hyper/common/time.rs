use crate::hyper::rt::Sleep;
use crate::hyper::rt::Timer;
use std::time::Duration;
use std::{fmt, sync::Arc};
use std::{pin::Pin, time::Instant};
#[derive(Clone)]
pub(crate) enum Time {
    Timer(Arc<dyn Timer + Send + Sync>),
    Empty,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Dur {
    Default(Option<Duration>),
    Configured(Option<Duration>),
}
impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Time").finish()
    }
}
impl Time {
    pub(crate) fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
        match &self {
            Time::Empty => {
                panic!("You must supply a timer.")
            }
            Time::Timer(t) => t.sleep(duration),
        }
    }
    pub(crate) fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
        match &self {
            Time::Empty => {
                panic!("You must supply a timer.")
            }
            Time::Timer(t) => t.sleep_until(deadline),
        }
    }
    pub(crate) fn now(&self) -> Instant {
        match &self {
            Time::Empty => Instant::now(),
            Time::Timer(t) => t.now(),
        }
    }
    pub(crate) fn reset(&self, sleep: &mut Pin<Box<dyn Sleep>>, new_deadline: Instant) {
        match &self {
            Time::Empty => {
                panic!("You must supply a timer.")
            }
            Time::Timer(t) => t.reset(sleep, new_deadline),
        }
    }
    pub(crate) fn check(&self, dur: Dur, name: &'static str) -> Option<Duration> {
        match dur {
            Dur::Default(Some(dur)) => match self {
                Time::Empty => {
                    warn!("timeout `{}` has default, but no timer set", name,);
                    None
                }
                Time::Timer(..) => Some(dur),
            },
            Dur::Configured(Some(dur)) => match self {
                Time::Empty => panic!("timeout `{name}` set, but no timer set",),
                Time::Timer(..) => Some(dur),
            },
            Dur::Default(None) | Dur::Configured(None) => None,
        }
    }
}
