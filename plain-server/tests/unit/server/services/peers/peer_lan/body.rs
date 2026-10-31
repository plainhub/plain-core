use bytes::Bytes;
use hyper::body::Frame;
use plain_rs::services::peers::peer_lan::limited_chunks;
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

enum Step {
    Data(&'static [u8]),
    Trailer,
    Pending,
    Failure,
}
struct Frames {
    steps: VecDeque<Step>,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl hyper::body::Body for Frames {
    type Data = Bytes;
    type Error = io::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let this = self.get_mut();
        this.calls.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(match this.steps.pop_front() {
            Some(Step::Data(data)) => Some(Ok(Frame::data(Bytes::from_static(data)))),
            Some(Step::Trailer) => Some(Ok(Frame::trailers(axum::http::HeaderMap::new()))),
            Some(Step::Failure) => Some(Err(io::Error::other("fixture body failure"))),
            Some(Step::Pending) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            None => None,
        })
    }
}
impl Drop for Frames {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
fn frames(steps: Vec<Step>) -> (Frames, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    (
        Frames {
            steps: steps.into(),
            calls: calls.clone(),
            drops: drops.clone(),
        },
        calls,
        drops,
    )
}

#[tokio::test]
async fn response_collection_preserves_pending_empty_data_trailers_and_exact_limits() {
    let (body, _, drops) = frames(vec![
        Step::Pending,
        Step::Data(b""),
        Step::Data(b"ab"),
        Step::Trailer,
        Step::Data(b"cd"),
    ]);
    assert_eq!(
        limited_chunks(axum::body::Body::new(body).into_data_stream(), 4)
            .await
            .unwrap(),
        b"abcd"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let (body, _, _) = frames(vec![Step::Data(b""), Step::Trailer]);
    assert!(
        limited_chunks(axum::body::Body::new(body).into_data_stream(), 0)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn collection_stops_on_overflow_or_the_original_body_error() {
    let (body, calls, drops) = frames(vec![Step::Data(b"ab"), Step::Data(b"unread")]);
    assert_eq!(
        limited_chunks(axum::body::Body::new(body).into_data_stream(), 1)
            .await
            .unwrap_err()
            .to_string(),
        "Peer response exceeds limit"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let (body, calls, _) = frames(vec![Step::Data(b"a"), Step::Failure, Step::Data(b"unread")]);
    assert!(
        limited_chunks(axum::body::Body::new(body).into_data_stream(), 8)
            .await
            .unwrap_err()
            .to_string()
            .contains("fixture body failure")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn canceling_collection_drops_the_underlying_body() {
    use std::future::Future;
    let (body, calls, drops) = frames(vec![Step::Pending, Step::Data(b"unread")]);
    let mut future = Box::pin(limited_chunks(
        axum::body::Body::new(body).into_data_stream(),
        8,
    ));
    let mut cx = Context::from_waker(std::task::Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(future);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
