use crate::hyper::rt::Timer as _;
use crate::hyper_util::common::{exec, exec::Exec, timer::Timer};
use futures_channel::oneshot;
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::Infallible;
use std::error::Error as StdError;
use std::fmt::{self, Debug};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{self, Poll, ready};
use std::time::{Duration, Instant};
use tracing::{debug, trace};
pub struct Pool<T, K: Key> {
    inner: Option<Arc<Mutex<PoolInner<T, K>>>>,
}
pub trait Poolable: Unpin + Send + Sized + 'static {
    fn is_open(&self) -> bool;
    fn reserve(self) -> Reservation<Self>;
    fn can_share(&self) -> bool;
}
pub trait Key: Eq + Hash + Clone + Debug + Unpin + Send + 'static {}
impl<T> Key for T where T: Eq + Hash + Clone + Debug + Unpin + Send + 'static {}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ver {
    Auto,
    Http2,
}
pub enum Reservation<T> {
    Shared(T, T),
    Unique(T),
}
struct PoolInner<T, K: Eq + Hash> {
    connecting: HashSet<K>,
    idle: HashMap<K, Vec<Idle<T>>>,
    max_idle_per_host: usize,
    waiters: HashMap<K, VecDeque<oneshot::Sender<T>>>,
    idle_interval_ref: Option<oneshot::Sender<Infallible>>,
    exec: Exec,
    timer: Option<Timer>,
    timeout: Option<Duration>,
}
struct WeakOpt<T>(Option<Weak<T>>);
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub idle_timeout: Option<Duration>,
    pub max_idle_per_host: usize,
}
impl Config {
    pub fn is_enabled(&self) -> bool {
        self.max_idle_per_host > 0
    }
}
impl<T, K: Key> Pool<T, K> {
    pub fn new<E, M>(config: Config, executor: E, timer: Option<M>) -> Pool<T, K>
    where
        E: crate::hyper::rt::Executor<exec::BoxSendFuture> + Send + Sync + Clone + 'static,
        M: crate::hyper::rt::Timer + Send + Sync + Clone + 'static,
    {
        let exec = Exec::new(executor);
        let timer = timer.map(|t| Timer::new(t));
        let inner = if config.is_enabled() {
            Some(Arc::new(Mutex::new(PoolInner {
                connecting: HashSet::new(),
                idle: HashMap::new(),
                idle_interval_ref: None,
                max_idle_per_host: config.max_idle_per_host,
                waiters: HashMap::new(),
                exec,
                timer,
                timeout: config.idle_timeout,
            })))
        } else {
            None
        };
        Pool { inner }
    }
    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }
}
impl<T: Poolable, K: Key> Pool<T, K> {
    pub fn checkout(&self, key: K) -> Checkout<T, K> {
        Checkout {
            key,
            pool: self.clone(),
            waiter: None,
        }
    }
    pub fn connecting(&self, key: &K, ver: Ver) -> Option<Connecting<T, K>> {
        if ver == Ver::Http2 {
            if let Some(ref enabled) = self.inner {
                let mut inner = enabled.lock().unwrap();
                return if inner.connecting.insert(key.clone()) {
                    let connecting = Connecting {
                        key: key.clone(),
                        pool: WeakOpt::downgrade(enabled),
                    };
                    Some(connecting)
                } else {
                    trace!("HTTP/2 connecting already in progress for {:?}", key);
                    None
                };
            }
        }
        Some(Connecting {
            key: key.clone(),
            pool: WeakOpt::none(),
        })
    }
    pub fn pooled(
        &self,
        #[cfg_attr(not(all()), allow(unused_mut))] mut connecting: Connecting<T, K>,
        value: T,
    ) -> Pooled<T, K> {
        let (value, pool_ref) = if let Some(ref enabled) = self.inner {
            match value.reserve() {
                Reservation::Shared(to_insert, to_return) => {
                    let mut inner = enabled.lock().unwrap();
                    inner.put(connecting.key.clone(), to_insert, enabled);
                    inner.connected(&connecting.key);
                    connecting.pool = WeakOpt::none();
                    (to_return, WeakOpt::none())
                }
                Reservation::Unique(value) => (value, WeakOpt::downgrade(enabled)),
            }
        } else {
            debug_assert!(connecting.pool.upgrade().is_none());
            (value, WeakOpt::none())
        };
        Pooled {
            key: connecting.key.clone(),
            is_reused: false,
            pool: pool_ref,
            value: Some(value),
        }
    }
    fn reuse(&self, key: &K, value: T) -> Pooled<T, K> {
        debug!("reuse idle connection for {:?}", key);
        let mut pool_ref = WeakOpt::none();
        if !value.can_share() {
            if let Some(ref enabled) = self.inner {
                pool_ref = WeakOpt::downgrade(enabled);
            }
        }
        Pooled {
            is_reused: true,
            key: key.clone(),
            pool: pool_ref,
            value: Some(value),
        }
    }
}
struct IdlePopper<'a, T, K> {
    key: &'a K,
    list: &'a mut Vec<Idle<T>>,
}
impl<'a, T: Poolable + 'a, K: Debug> IdlePopper<'a, T, K> {
    fn pop(self, expiration: &Expiration, now: Instant) -> Option<Idle<T>> {
        while let Some(entry) = self.list.pop() {
            if !entry.value.is_open() {
                trace!("removing closed connection for {:?}", self.key);
                continue;
            }
            if expiration.expires(entry.idle_at, now) {
                trace!("removing expired connection for {:?}", self.key);
                continue;
            }
            let value = match entry.value.reserve() {
                Reservation::Shared(to_reinsert, to_checkout) => {
                    self.list.push(Idle {
                        idle_at: now,
                        value: to_reinsert,
                    });
                    to_checkout
                }
                Reservation::Unique(unique) => unique,
            };
            return Some(Idle {
                idle_at: entry.idle_at,
                value,
            });
        }
        None
    }
}
impl<T: Poolable, K: Key> PoolInner<T, K> {
    fn now(&self) -> Instant {
        self.timer
            .as_ref()
            .map_or_else(|| Instant::now(), |t| t.now())
    }
    fn put(&mut self, key: K, value: T, __pool_ref: &Arc<Mutex<PoolInner<T, K>>>) {
        if value.can_share() && self.idle.contains_key(&key) {
            trace!("put; existing idle HTTP/2 connection for {:?}", key);
            return;
        }
        trace!("put; add idle connection for {:?}", key);
        let mut remove_waiters = false;
        let mut value = Some(value);
        if let Some(waiters) = self.waiters.get_mut(&key) {
            while let Some(tx) = waiters.pop_front() {
                if !tx.is_canceled() {
                    let reserved = value.take().expect("value already sent");
                    let reserved = match reserved.reserve() {
                        Reservation::Shared(to_keep, to_send) => {
                            value = Some(to_keep);
                            to_send
                        }
                        Reservation::Unique(uniq) => uniq,
                    };
                    match tx.send(reserved) {
                        Ok(()) => {
                            if value.is_none() {
                                break;
                            } else {
                                continue;
                            }
                        }
                        Err(e) => {
                            value = Some(e);
                        }
                    }
                }
                trace!("put; removing canceled waiter for {:?}", key);
            }
            remove_waiters = waiters.is_empty();
        }
        if remove_waiters {
            self.waiters.remove(&key);
        }
        match value {
            Some(value) => {
                {
                    let now = self.now();
                    let idle_list = self.idle.entry(key.clone()).or_default();
                    if self.max_idle_per_host <= idle_list.len() {
                        trace!("max idle per host for {:?}, dropping connection", key);
                        return;
                    }
                    debug!("pooling idle connection for {:?}", key);
                    idle_list.push(Idle {
                        value,
                        idle_at: now,
                    });
                }
                self.spawn_idle_interval(__pool_ref);
            }
            None => trace!("put; found waiter for {:?}", key),
        }
    }
    fn connected(&mut self, key: &K) {
        let existed = self.connecting.remove(key);
        debug_assert!(existed, "Connecting dropped, key not in pool.connecting");
        self.waiters.remove(key);
    }
    fn spawn_idle_interval(&mut self, pool_ref: &Arc<Mutex<PoolInner<T, K>>>) {
        if self.idle_interval_ref.is_some() {
            return;
        }
        let dur = if let Some(dur) = self.timeout {
            dur
        } else {
            return;
        };
        if dur == Duration::ZERO {
            return;
        }
        let timer = if let Some(timer) = self.timer.clone() {
            timer
        } else {
            return;
        };
        const MIN_CHECK: Duration = Duration::from_millis(90);
        let dur = dur.max(MIN_CHECK);
        let (tx, rx) = oneshot::channel();
        self.idle_interval_ref = Some(tx);
        let interval = IdleTask {
            timer: timer.clone(),
            duration: dur,
            pool: WeakOpt::downgrade(pool_ref),
            pool_drop_notifier: rx,
        };
        self.exec.execute(interval.run());
    }
}
impl<T, K: Eq + Hash> PoolInner<T, K> {
    fn clean_waiters(&mut self, key: &K) {
        let mut remove_waiters = false;
        if let Some(waiters) = self.waiters.get_mut(key) {
            waiters.retain(|tx| !tx.is_canceled());
            remove_waiters = waiters.is_empty();
        }
        if remove_waiters {
            self.waiters.remove(key);
        }
    }
}
impl<T: Poolable, K: Key> PoolInner<T, K> {
    fn clear_expired(&mut self) {
        let dur = self.timeout.expect("interval assumes timeout");
        let now = self.now();
        self.idle.retain(|key, values| {
            values.retain(|entry| {
                if !entry.value.is_open() {
                    trace!("idle interval evicting closed for {:?}", key);
                    return false;
                }
                if now.saturating_duration_since(entry.idle_at) > dur {
                    trace!("idle interval evicting expired for {:?}", key);
                    return false;
                }
                true
            });
            !values.is_empty()
        });
    }
}
impl<T, K: Key> Clone for Pool<T, K> {
    fn clone(&self) -> Pool<T, K> {
        Pool {
            inner: self.inner.clone(),
        }
    }
}
pub struct Pooled<T: Poolable, K: Key> {
    value: Option<T>,
    is_reused: bool,
    key: K,
    pool: WeakOpt<Mutex<PoolInner<T, K>>>,
}
impl<T: Poolable, K: Key> Pooled<T, K> {
    pub fn is_reused(&self) -> bool {
        self.is_reused
    }
    pub fn is_pool_enabled(&self) -> bool {
        self.pool.0.is_some()
    }
    fn as_ref(&self) -> &T {
        self.value.as_ref().expect("not dropped")
    }
    fn as_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("not dropped")
    }
}
impl<T: Poolable, K: Key> Deref for Pooled<T, K> {
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}
impl<T: Poolable, K: Key> DerefMut for Pooled<T, K> {
    fn deref_mut(&mut self) -> &mut T {
        self.as_mut()
    }
}
impl<T: Poolable, K: Key> Drop for Pooled<T, K> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            if !value.is_open() {
                return;
            }
            if let Some(pool) = self.pool.upgrade() {
                if let Ok(mut inner) = pool.lock() {
                    inner.put(self.key.clone(), value, &pool);
                }
            } else if !value.can_share() {
                trace!("pool dropped, dropping pooled ({:?})", self.key);
            }
        }
    }
}
impl<T: Poolable, K: Key> fmt::Debug for Pooled<T, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pooled").field("key", &self.key).finish()
    }
}
struct Idle<T> {
    idle_at: Instant,
    value: T,
}
pub struct Checkout<T, K: Key> {
    key: K,
    pool: Pool<T, K>,
    waiter: Option<oneshot::Receiver<T>>,
}
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    PoolDisabled,
    CheckoutNoLongerWanted,
    CheckedOutClosedValue,
}
impl Error {
    pub(super) fn is_canceled(&self) -> bool {
        matches!(self, Error::CheckedOutClosedValue)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::PoolDisabled => "pool is disabled",
            Error::CheckedOutClosedValue => "checked out connection was closed",
            Error::CheckoutNoLongerWanted => "request was canceled",
        })
    }
}
impl StdError for Error {}
impl<T: Poolable, K: Key> Checkout<T, K> {
    fn poll_waiter(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Pooled<T, K>, Error>>> {
        if let Some(mut rx) = self.waiter.take() {
            match Pin::new(&mut rx).poll(cx) {
                Poll::Ready(Ok(value)) => {
                    if value.is_open() {
                        Poll::Ready(Some(Ok(self.pool.reuse(&self.key, value))))
                    } else {
                        Poll::Ready(Some(Err(Error::CheckedOutClosedValue)))
                    }
                }
                Poll::Pending => {
                    self.waiter = Some(rx);
                    Poll::Pending
                }
                Poll::Ready(Err(_canceled)) => {
                    Poll::Ready(Some(Err(Error::CheckoutNoLongerWanted)))
                }
            }
        } else {
            Poll::Ready(None)
        }
    }
    fn checkout(&mut self, cx: &mut task::Context<'_>) -> Option<Pooled<T, K>> {
        let entry = {
            let mut inner = self.pool.inner.as_ref()?.lock().unwrap();
            let expiration = Expiration::new(inner.timeout);
            let now = inner.now();
            let maybe_entry = inner.idle.get_mut(&self.key).and_then(|list| {
                trace!("take? {:?}: expiration = {:?}", self.key, expiration.0);
                {
                    let popper = IdlePopper {
                        key: &self.key,
                        list,
                    };
                    popper.pop(&expiration, now)
                }
                .map(|e| (e, list.is_empty()))
            });
            let (entry, empty) = if let Some((e, empty)) = maybe_entry {
                (Some(e), empty)
            } else {
                (None, true)
            };
            if empty {
                inner.idle.remove(&self.key);
            }
            if entry.is_none() && self.waiter.is_none() {
                let (tx, mut rx) = oneshot::channel();
                trace!("checkout waiting for idle connection: {:?}", self.key);
                inner
                    .waiters
                    .entry(self.key.clone())
                    .or_insert_with(VecDeque::new)
                    .push_back(tx);
                assert!(Pin::new(&mut rx).poll(cx).is_pending());
                self.waiter = Some(rx);
            }
            entry
        };
        entry.map(|e| self.pool.reuse(&self.key, e.value))
    }
}
impl<T: Poolable, K: Key> Future for Checkout<T, K> {
    type Output = Result<Pooled<T, K>, Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        if let Some(pooled) = ready!(self.poll_waiter(cx)?) {
            return Poll::Ready(Ok(pooled));
        }
        if let Some(pooled) = self.checkout(cx) {
            Poll::Ready(Ok(pooled))
        } else if !self.pool.is_enabled() {
            Poll::Ready(Err(Error::PoolDisabled))
        } else {
            debug_assert!(self.waiter.is_some());
            Poll::Pending
        }
    }
}
impl<T, K: Key> Drop for Checkout<T, K> {
    fn drop(&mut self) {
        if self.waiter.take().is_some() {
            trace!("checkout dropped for {:?}", self.key);
            if let Some(Ok(mut inner)) = self.pool.inner.as_ref().map(|i| i.lock()) {
                inner.clean_waiters(&self.key);
            }
        }
    }
}
pub struct Connecting<T: Poolable, K: Key> {
    key: K,
    pool: WeakOpt<Mutex<PoolInner<T, K>>>,
}
impl<T: Poolable, K: Key> Connecting<T, K> {
    pub fn alpn_h2(self, pool: &Pool<T, K>) -> Option<Self> {
        debug_assert!(
            self.pool.0.is_none(),
            "Connecting::alpn_h2 but already Http2"
        );
        pool.connecting(&self.key, Ver::Http2)
    }
}
impl<T: Poolable, K: Key> Drop for Connecting<T, K> {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            if let Ok(mut inner) = pool.lock() {
                inner.connected(&self.key);
            }
        }
    }
}
struct Expiration(Option<Duration>);
impl Expiration {
    fn new(dur: Option<Duration>) -> Expiration {
        Expiration(dur)
    }
    fn expires(&self, instant: Instant, now: Instant) -> bool {
        match self.0 {
            Some(timeout) => now.saturating_duration_since(instant) > timeout,
            None => false,
        }
    }
}
struct IdleTask<T, K: Key> {
    timer: Timer,
    duration: Duration,
    pool: WeakOpt<Mutex<PoolInner<T, K>>>,
    pool_drop_notifier: oneshot::Receiver<Infallible>,
}
impl<T: Poolable + 'static, K: Key> IdleTask<T, K> {
    async fn run(self) {
        use futures_util::future;
        let mut sleep = self.timer.sleep_until(self.timer.now() + self.duration);
        let mut on_pool_drop = self.pool_drop_notifier;
        loop {
            match future::select(&mut on_pool_drop, &mut sleep).await {
                future::Either::Left(_) => {
                    break;
                }
                future::Either::Right(((), _)) => {
                    if let Some(inner) = self.pool.upgrade() {
                        if let Ok(mut inner) = inner.lock() {
                            trace!("idle interval checking for expired");
                            inner.clear_expired();
                            if inner.idle.is_empty() {
                                inner.idle_interval_ref = None;
                                trace!("pool empty, canceling idle interval");
                                return;
                            }
                        }
                    }
                    let deadline = self.timer.now() + self.duration;
                    self.timer.reset(&mut sleep, deadline);
                }
            }
        }
        trace!("pool closed, canceling idle interval");
        return;
    }
}
impl<T> WeakOpt<T> {
    fn none() -> Self {
        WeakOpt(None)
    }
    fn downgrade(arc: &Arc<T>) -> Self {
        WeakOpt(Some(Arc::downgrade(arc)))
    }
    fn upgrade(&self) -> Option<Arc<T>> {
        self.0.as_ref().and_then(Weak::upgrade)
    }
}
