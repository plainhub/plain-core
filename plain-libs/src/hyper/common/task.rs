use std::task::{Context, Poll};
use std::task::{RawWaker, RawWakerVTable, Waker};
pub(crate) fn yield_now(cx: &mut Context<'_>) -> Poll<std::convert::Infallible> {
    cx.waker().wake_by_ref();
    Poll::Pending
}
fn noop_waker() -> Waker {
    const NOOP_RAW_WAKER: RawWaker = RawWaker::new(std::ptr::null(), &NOOP_VTABLE);
    const NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_: *const ()| NOOP_RAW_WAKER,
        |_: *const ()| {},
        |_: *const ()| {},
        |_: *const ()| {},
    );
    unsafe { Waker::from_raw(NOOP_RAW_WAKER) }
}
pub(crate) fn now_or_never<F: std::future::Future>(fut: F) -> Option<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    tokio::pin!(fut);
    match fut.poll(&mut cx) {
        Poll::Ready(res) => Some(res),
        Poll::Pending => None,
    }
}
