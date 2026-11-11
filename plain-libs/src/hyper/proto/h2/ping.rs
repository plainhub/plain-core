use crate::h2::{Ping, PingPong};
use crate::hyper::common::lock::LockResultExt;
use crate::hyper::common::time::Time;
use crate::hyper::rt::Sleep;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{self, Poll};
use std::time::{Duration, Instant};
type WindowSize = u32;
pub(super) fn disabled() -> Recorder {
    Recorder { shared: None }
}
pub(super) fn channel(ping_pong: PingPong, config: Config, timer: Time) -> (Recorder, Ponger) {
    debug_assert!(
        config.is_enabled(),
        "ping channel requires bdp or keep-alive config",
    );
    let bdp = config.bdp_initial_window.map(|wnd| Bdp {
        bdp: wnd,
        max_bandwidth: 0.0,
        rtt: 0.0,
        ping_delay: Duration::from_millis(100),
        stable_count: 0,
    });
    let now = timer.now();
    let (bytes, next_bdp_at) = if bdp.is_some() {
        (Some(0), Some(now))
    } else {
        (None, None)
    };
    let keep_alive = config.keep_alive_interval.map(|interval| KeepAlive {
        interval,
        timeout: config.keep_alive_timeout,
        while_idle: config.keep_alive_while_idle,
        sleep: timer.sleep(interval),
        state: KeepAliveState::Init,
        timer: timer.clone(),
    });
    let last_read_at = keep_alive.as_ref().map(|_| now);
    let shared = Arc::new(Mutex::new(Shared {
        bytes,
        last_read_at,
        is_keep_alive_timed_out: false,
        ping_pong,
        ping_sent_at: None,
        next_bdp_at,
        timer,
    }));
    (
        Recorder {
            shared: Some(shared.clone()),
        },
        Ponger {
            bdp,
            keep_alive,
            shared,
        },
    )
}
#[derive(Clone)]
pub(super) struct Config {
    pub(super) bdp_initial_window: Option<WindowSize>,
    pub(super) keep_alive_interval: Option<Duration>,
    pub(super) keep_alive_timeout: Duration,
    pub(super) keep_alive_while_idle: bool,
}
#[derive(Clone)]
pub(crate) struct Recorder {
    shared: Option<Arc<Mutex<Shared>>>,
}
pub(super) struct Ponger {
    bdp: Option<Bdp>,
    keep_alive: Option<KeepAlive>,
    shared: Arc<Mutex<Shared>>,
}
struct Shared {
    ping_pong: PingPong,
    ping_sent_at: Option<Instant>,
    bytes: Option<usize>,
    next_bdp_at: Option<Instant>,
    last_read_at: Option<Instant>,
    is_keep_alive_timed_out: bool,
    timer: Time,
}
struct Bdp {
    bdp: u32,
    max_bandwidth: f64,
    rtt: f64,
    ping_delay: Duration,
    stable_count: u32,
}
struct KeepAlive {
    interval: Duration,
    timeout: Duration,
    while_idle: bool,
    state: KeepAliveState,
    sleep: Pin<Box<dyn Sleep>>,
    timer: Time,
}
enum KeepAliveState {
    Init,
    Scheduled(Instant),
    PingSent,
}
pub(super) enum Ponged {
    SizeUpdate(WindowSize),
    KeepAliveTimedOut,
}
#[derive(Debug)]
pub(super) struct KeepAliveTimedOut;
impl Config {
    pub(super) fn is_enabled(&self) -> bool {
        self.bdp_initial_window.is_some() || self.keep_alive_interval.is_some()
    }
}
impl Recorder {
    pub(crate) fn record_data(&self, len: usize) {
        let shared = if let Some(shared) = &self.shared {
            shared
        } else {
            return;
        };
        let mut locked = shared.lock().panic_if_poisoned();
        locked.update_last_read_at();
        if let Some(next_bdp_at) = &locked.next_bdp_at {
            if locked.timer.now() < *next_bdp_at {
                return;
            } else {
                locked.next_bdp_at = None;
            }
        }
        if let Some(bytes) = &mut locked.bytes {
            *bytes += len;
        } else {
            return;
        }
        if !locked.is_ping_sent() {
            locked.send_ping();
        }
    }
    pub(crate) fn record_non_data(&self) {
        let shared = if let Some(shared) = &self.shared {
            shared
        } else {
            return;
        };
        let mut locked = shared.lock().panic_if_poisoned();
        locked.update_last_read_at();
    }
    pub(super) fn for_stream(self, stream: &crate::h2::RecvStream) -> Self {
        if stream.is_end_stream() {
            disabled()
        } else {
            self
        }
    }
    pub(super) fn ensure_not_timed_out(&self) -> crate::hyper::Result<()> {
        if let Some(shared) = &self.shared {
            let locked = shared.lock().panic_if_poisoned();
            if locked.is_keep_alive_timed_out {
                return Err(KeepAliveTimedOut.crate_error());
            }
        }
        Ok(())
    }
}
impl Ponger {
    pub(super) fn poll(&mut self, cx: &mut task::Context<'_>) -> Poll<Ponged> {
        let mut locked = self.shared.lock().panic_if_poisoned();
        let now = locked.timer.now();
        let is_idle = self.is_idle();
        if let Some(ka) = &mut self.keep_alive {
            ka.maybe_schedule(is_idle, &locked);
            ka.maybe_ping(cx, is_idle, &mut locked);
        }
        if !locked.is_ping_sent() {
            return Poll::Pending;
        }
        match locked.ping_pong.poll_pong(cx) {
            Poll::Ready(Ok(_pong)) => {
                let start = locked
                    .ping_sent_at
                    .expect("pong received implies ping_sent_at");
                locked.ping_sent_at = None;
                let rtt = now - start;
                trace!("recv pong");
                if let Some(ka) = &mut self.keep_alive {
                    locked.update_last_read_at();
                    ka.maybe_schedule(is_idle, &locked);
                    ka.maybe_ping(cx, is_idle, &mut locked);
                }
                if let Some(bdp) = &mut self.bdp {
                    let bytes = locked.bytes.expect("bdp enabled implies bytes");
                    locked.bytes = Some(0);
                    trace!("received BDP ack; bytes = {}, rtt = {:?}", bytes, rtt);
                    let update = bdp.calculate(bytes, rtt);
                    locked.next_bdp_at = Some(now + bdp.ping_delay);
                    if let Some(update) = update {
                        return Poll::Ready(Ponged::SizeUpdate(update));
                    }
                }
            }
            Poll::Ready(Err(_e)) => {
                debug!("pong error: {}", _e);
            }
            Poll::Pending => {
                if let Some(ka) = &mut self.keep_alive {
                    if let Err(KeepAliveTimedOut) = ka.maybe_timeout(cx) {
                        self.keep_alive = None;
                        locked.is_keep_alive_timed_out = true;
                        return Poll::Ready(Ponged::KeepAliveTimedOut);
                    }
                }
            }
        }
        Poll::Pending
    }
    fn is_idle(&self) -> bool {
        Arc::strong_count(&self.shared) <= 2
    }
}
impl Shared {
    fn send_ping(&mut self) {
        match self.ping_pong.send_ping(Ping::opaque()) {
            Ok(()) => {
                self.ping_sent_at = Some(self.timer.now());
                trace!("sent ping");
            }
            Err(_err) => {
                debug!("error sending ping: {}", _err);
            }
        }
    }
    fn is_ping_sent(&self) -> bool {
        self.ping_sent_at.is_some()
    }
    fn update_last_read_at(&mut self) {
        if self.last_read_at.is_some() {
            self.last_read_at = Some(self.timer.now());
        }
    }
    fn last_read_at(&self) -> Instant {
        self.last_read_at.expect("keep_alive expects last_read_at")
    }
}
const BDP_LIMIT: usize = 1024 * 1024 * 16;
impl Bdp {
    fn calculate(&mut self, bytes: usize, rtt: Duration) -> Option<WindowSize> {
        if self.bdp as usize == BDP_LIMIT {
            self.stabilize_delay();
            return None;
        }
        let rtt = seconds(rtt);
        if self.rtt == 0.0 {
            self.rtt = rtt;
        } else {
            self.rtt += (rtt - self.rtt) * 0.125;
        }
        let bw = (bytes as f64) / (self.rtt * 1.5);
        trace!("current bandwidth = {:.1}B/s", bw);
        if bw < self.max_bandwidth {
            self.stabilize_delay();
            return None;
        } else {
            self.max_bandwidth = bw;
        }
        if bytes >= self.bdp as usize * 2 / 3 {
            self.bdp = (bytes * 2).min(BDP_LIMIT) as WindowSize;
            trace!("BDP increased to {}", self.bdp);
            self.stable_count = 0;
            self.ping_delay /= 2;
            Some(self.bdp)
        } else {
            self.stabilize_delay();
            None
        }
    }
    fn stabilize_delay(&mut self) {
        if self.ping_delay < Duration::from_secs(10) {
            self.stable_count += 1;
            if self.stable_count >= 2 {
                self.ping_delay *= 4;
                self.stable_count = 0;
            }
        }
    }
}
fn seconds(dur: Duration) -> f64 {
    const NANOS_PER_SEC: f64 = 1_000_000_000.0;
    let secs = dur.as_secs() as f64;
    secs + (f64::from(dur.subsec_nanos())) / NANOS_PER_SEC
}
impl KeepAlive {
    fn maybe_schedule(&mut self, is_idle: bool, shared: &Shared) {
        match self.state {
            KeepAliveState::Init => {
                if !self.while_idle && is_idle {
                    return;
                }
                self.schedule(shared);
            }
            KeepAliveState::PingSent => {
                if shared.is_ping_sent() {
                    return;
                }
                self.schedule(shared);
            }
            KeepAliveState::Scheduled(..) => (),
        }
    }
    fn schedule(&mut self, shared: &Shared) {
        let interval = shared.last_read_at() + self.interval;
        self.state = KeepAliveState::Scheduled(interval);
        self.timer.reset(&mut self.sleep, interval);
    }
    fn maybe_ping(&mut self, cx: &mut task::Context<'_>, is_idle: bool, shared: &mut Shared) {
        match self.state {
            KeepAliveState::Scheduled(at) => {
                if Pin::new(&mut self.sleep).poll(cx).is_pending() {
                    return;
                }
                if shared.last_read_at() + self.interval > at {
                    self.state = KeepAliveState::Init;
                    cx.waker().wake_by_ref();
                    return;
                }
                if !self.while_idle && is_idle {
                    trace!("keep-alive no need to ping when idle and while_idle=false");
                    return;
                }
                trace!("keep-alive interval ({:?}) reached", self.interval);
                shared.send_ping();
                self.state = KeepAliveState::PingSent;
                let timeout = self.timer.now() + self.timeout;
                self.timer.reset(&mut self.sleep, timeout);
            }
            KeepAliveState::Init | KeepAliveState::PingSent => (),
        }
    }
    fn maybe_timeout(&mut self, cx: &mut task::Context<'_>) -> Result<(), KeepAliveTimedOut> {
        match self.state {
            KeepAliveState::PingSent => {
                if Pin::new(&mut self.sleep).poll(cx).is_pending() {
                    return Ok(());
                }
                trace!("keep-alive timeout ({:?}) reached", self.timeout);
                Err(KeepAliveTimedOut)
            }
            KeepAliveState::Init | KeepAliveState::Scheduled(..) => Ok(()),
        }
    }
}
impl KeepAliveTimedOut {
    pub(super) fn crate_error(self) -> crate::hyper::Error {
        crate::hyper::Error::new(crate::hyper::error::Kind::Http2).with(self)
    }
}
impl fmt::Display for KeepAliveTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("keep-alive timed out")
    }
}
impl std::error::Error for KeepAliveTimedOut {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&crate::hyper::error::TimedOut)
    }
}
