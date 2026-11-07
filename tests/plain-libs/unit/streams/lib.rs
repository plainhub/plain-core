use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Wake, Waker};

#[derive(Default)]
struct Counter(AtomicUsize);

impl Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn poll<S: Stream>(stream: Pin<&mut S>) -> Poll<Option<S::Item>> {
    let waker = Waker::from(Arc::new(Counter::default()));
    stream.poll_next(&mut Context::from_waker(&waker))
}

#[test]
fn factory_is_lazy_and_each_item_requires_consumer_progress() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut stream = Box::pin(stream_fn(move |mut output| {
        observed.fetch_add(1, Ordering::SeqCst);
        async move {
            output.yield_item(10).await;
            observed.fetch_add(1, Ordering::SeqCst);
            output.yield_item(20).await;
        }
    }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(poll(stream.as_mut()), Poll::Ready(Some(10)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(poll(stream.as_mut()), Poll::Ready(Some(20)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(poll(stream.as_mut()), Poll::Ready(None));
    assert_eq!(poll(stream.as_mut()), Poll::Ready(None));
}

#[test]
fn pending_producer_retains_the_consumers_waker() {
    let saved = Arc::new(Mutex::new(None::<Waker>));
    let entered = Arc::clone(&saved);
    let mut stream = Box::pin(stream_fn(move |mut output| async move {
        poll_fn(|cx| {
            let mut waker = entered.lock().unwrap();
            if waker.is_some() {
                Poll::Ready(())
            } else {
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await;
        output.yield_item(7).await;
    }));
    let counter = Arc::new(Counter::default());
    let waker = Waker::from(Arc::clone(&counter));
    assert_eq!(
        stream.as_mut().poll_next(&mut Context::from_waker(&waker)),
        Poll::Pending
    );
    saved.lock().unwrap().as_ref().unwrap().wake_by_ref();
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    assert_eq!(poll(stream.as_mut()), Poll::Ready(Some(7)));
    assert_eq!(poll(stream.as_mut()), Poll::Ready(None));
}

#[test]
fn dropping_after_yield_cancels_the_producer_and_drops_owned_state() {
    let drops = Arc::new(AtomicUsize::new(0));
    let probe = DropProbe(Arc::clone(&drops));
    let mut stream = Box::pin(stream_fn(move |mut output| async move {
        let _probe = probe;
        output.yield_item(1).await;
        std::future::pending::<()>().await;
    }));
    assert_eq!(poll(stream.as_mut()), Poll::Ready(Some(1)));
    drop(stream);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn nested_streams_and_borrowed_values_keep_separate_output_slots() {
    let value = String::from("borrowed");
    let value_ref = value.as_str();
    let mut outer = Box::pin(stream_fn(|mut output| async move {
        let mut inner = Box::pin(stream_fn(|mut inner| async move {
            inner.yield_item(value_ref).await;
            inner.yield_item("second").await;
        }));
        while let Some(item) = poll_fn(|cx| inner.as_mut().poll_next(cx)).await {
            output.yield_item(item).await;
        }
    }));
    assert_eq!(poll(outer.as_mut()), Poll::Ready(Some("borrowed")));
    assert_eq!(poll(outer.as_mut()), Poll::Ready(Some("second")));
    assert_eq!(poll(outer.as_mut()), Poll::Ready(None));
}

#[test]
fn dropping_before_first_poll_drops_the_unstarted_factory() {
    let drops = Arc::new(AtomicUsize::new(0));
    let probe = DropProbe(Arc::clone(&drops));
    let stream = stream_fn(move |mut output| async move {
        let _probe = probe;
        output.yield_item(1).await;
    });
    drop(stream);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
