use super::*;
#[path = "../../support/poll.rs"]
mod support;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn zero_buffer_keeps_one_reserved_slot_per_sender_and_unparks_on_receive() {
    let (mut tx, mut rx) = channel(0);
    let (count, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    assert!(tx.poll_ready(&mut cx).is_ready());
    tx.try_send(11).unwrap();
    let error = tx.try_send(12).unwrap_err();
    assert_eq!(error.to_string(), "send failed because channel is full");
    assert_eq!(error.into_inner(), 12);
    assert!(tx.poll_ready(&mut cx).is_pending());
    assert_eq!(Pin::new(&mut rx).poll_next(&mut cx), Poll::Ready(Some(11)));
    assert_eq!(count.count(), 1);
    assert!(tx.poll_ready(&mut cx).is_ready());
    tx.try_send(13).unwrap();
    let mut other = tx.clone();
    other.try_send(14).unwrap();
    drop(tx);
    drop(other);
    assert_eq!(Pin::new(&mut rx).poll_next(&mut cx), Poll::Ready(Some(13)));
    assert_eq!(Pin::new(&mut rx).poll_next(&mut cx), Poll::Ready(Some(14)));
    assert_eq!(Pin::new(&mut rx).poll_next(&mut cx), Poll::Ready(None));
    assert!(rx.is_terminated());
    assert_eq!(rx.size_hint(), (0, Some(0)));
}

#[test]
fn bounded_messages_keep_fifo_order_and_allocation_identity() {
    let (mut tx, mut rx) = channel(4);
    let (_, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    let a = vec![1; 128];
    let b = vec![2; 256];
    let pointers = [a.as_ptr(), b.as_ptr()];
    tx.try_send(a).unwrap();
    tx.try_send(b).unwrap();
    assert_eq!(rx.size_hint(), (2, None));
    for (i, ptr) in pointers.into_iter().enumerate() {
        let Poll::Ready(Some(data)) = Pin::new(&mut rx).poll_next(&mut cx) else {
            panic!("missing message");
        };
        assert_eq!(data.as_ptr(), ptr);
        assert!(data.iter().all(|x| *x == (i + 1) as u8));
    }
}

struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn receiver_drop_drains_messages_and_wakes_parked_senders() {
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut tx, rx) = channel(0);
    let (count, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    tx.try_send(Dropped(drops.clone()))
        .unwrap_or_else(|_| panic!("send failed"));
    assert!(tx.poll_ready(&mut cx).is_pending());
    drop(rx);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(count.count(), 1);
    let Poll::Ready(Err(error)) = tx.poll_ready(&mut cx) else {
        panic!("sender should be closed");
    };
    assert_eq!(error.to_string(), "send failed because receiver is gone");
    let error = tx.try_send(Dropped(drops.clone())).err().unwrap();
    assert_eq!(error.to_string(), "send failed because receiver is gone");
    drop(error.into_inner());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multiple_producers_deliver_every_message_in_each_producer_order() {
    let (tx, mut rx) = channel(7);
    let mut workers = Vec::new();
    for producer in 0..8 {
        let mut tx = tx.clone();
        workers.push(tokio::spawn(async move {
            for sequence in 0..128 {
                std::future::poll_fn(|cx| tx.poll_ready(cx)).await.unwrap();
                tx.try_send((producer, sequence)).unwrap();
            }
        }));
    }
    drop(tx);
    let mut expected = [0; 8];
    while let Some((producer, sequence)) =
        std::future::poll_fn(|cx| Pin::new(&mut rx).poll_next(cx)).await
    {
        assert_eq!(sequence, expected[producer]);
        expected[producer] += 1;
    }
    for worker in workers {
        worker.await.unwrap();
    }
    assert_eq!(expected, [128; 8]);
}

#[test]
fn explicit_sender_close_wakes_waiters_and_drains_all_already_queued_bytes() {
    let (mut tx, mut rx) = channel(0);
    let (count, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    tx.start_send(vec![1, 2, 3]).unwrap();
    assert!(tx.poll_ready(&mut cx).is_pending());
    tx.close_channel();
    assert_eq!(
        Pin::new(&mut rx).poll_next(&mut cx),
        Poll::Ready(Some(vec![1, 2, 3]))
    );
    assert_eq!(count.count(), 1);
    assert_eq!(Pin::new(&mut rx).poll_next(&mut cx), Poll::Ready(None));
    assert!(
        matches!(tx.start_send(vec![4]), Err(ref error) if error.to_string() == "send failed because receiver is gone")
    );
}

#[test]
#[ignore = "Requires PLAIN_LIBS_CODEGEN_FUTURES_CORE and PLAIN_LIBS_CODEGEN_DEPS"]
fn sender_has_no_absent_state_or_duplicate_readiness_check() {
    use crate::codegen_test::{compile_probe_externs, function_body};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = format!(
        r#"extern crate alloc;
#[path = {atomic:?}] mod atomic_waker;
#[path = {channel:?}] mod async_channel;
use futures_core::Stream;
#[unsafe(no_mangle)]
pub fn receiver_poll(receiver: &mut async_channel::mpsc::Receiver<usize>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<usize>> {{ std::pin::Pin::new(receiver).poll_next(cx) }}
#[unsafe(no_mangle)]
pub fn sender_closed(sender: &async_channel::mpsc::Sender<usize>) -> bool {{ sender.is_closed() }}
#[unsafe(no_mangle)]
pub fn sender_send(sender: &mut async_channel::mpsc::Sender<usize>, value: usize) -> bool {{ sender.try_send(value).is_ok() }}
"#,
        atomic = root.join("atomic_waker/mod.rs"),
        channel = root.join("async_channel/mod.rs")
    );
    let library =
        std::env::var_os("PLAIN_LIBS_CODEGEN_FUTURES_CORE").expect("futures-core rlib required");
    let dependencies =
        std::env::var_os("PLAIN_LIBS_CODEGEN_DEPS").expect("dependency directory required");
    let ir = compile_probe_externs(
        &source,
        &[("futures_core", library.into())],
        dependencies.into(),
    );
    assert!(
        ir.lines()
            .any(|line| (line.contains("call ") || line.contains("invoke "))
                && line.contains("SenderTask")
                && line.contains("notify")
                && !line.trim_start().starts_with(';'))
    );
    assert!(
        !function_body(&ir, "sender_closed")
            .iter()
            .any(|line| line.contains("br i1") || line.contains("switch "))
    );
    let readiness_calls = ir
        .lines()
        .filter(|line| {
            (line.contains("call ") || line.contains("invoke "))
                && line.contains("poll_unparked")
                && !line.trim_start().starts_with(';')
        })
        .count();
    assert!(
        readiness_calls <= 1,
        "duplicate readiness checks: {readiness_calls}"
    );
}
