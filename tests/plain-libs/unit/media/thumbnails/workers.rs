use super::*;
use std::{collections::HashSet, future::Future, sync::Barrier};

fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> std::task::Poll<F::Output> {
    future.poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

#[tokio::test]
async fn repeated_batches_reuse_exactly_the_configured_threads() {
    let workers = Workers::new(3).unwrap();
    let mut original = None;
    for _ in 0..10 {
        let barrier = Arc::new(Barrier::new(4));
        let mut pending = Vec::new();
        for _ in 0..3 {
            let barrier = barrier.clone();
            let mut task = Box::pin(workers.run(move || {
                barrier.wait();
                assert!(
                    std::thread::current()
                        .name()
                        .unwrap()
                        .starts_with("thumbnail-")
                );
                std::thread::current().id()
            }));
            assert!(poll_once(task.as_mut()).is_pending());
            pending.push(task);
        }
        barrier.wait();
        let mut ids = HashSet::new();
        for task in pending {
            ids.insert(task.await.unwrap());
        }
        assert_eq!(ids.len(), 3);
        assert_eq!(original.get_or_insert_with(|| ids.clone()), &ids);
    }
}

#[tokio::test]
async fn panic_is_reported_and_the_worker_remains_available() {
    let workers = Workers::new(1).unwrap();
    let before = workers.run(|| std::thread::current().id()).await.unwrap();
    let error = workers.run(|| panic!("decode failed")).await.unwrap_err();
    assert!(error.to_string().contains("decode failed"));
    assert_eq!(
        workers.run(|| std::thread::current().id()).await.unwrap(),
        before
    );
}

#[tokio::test]
async fn cancelled_queued_job_releases_captures_without_execution() {
    let workers = Workers::new(1).unwrap();
    let (started, running) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let mut first = Box::pin(workers.run(move || {
        started.send(()).unwrap();
        wait.recv().unwrap();
    }));
    assert!(poll_once(first.as_mut()).is_pending());
    running.recv().unwrap();
    let capture = Arc::new(vec![1_u8; 4096]);
    let weak = Arc::downgrade(&capture);
    let executions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let executed = executions.clone();
    let mut second = Box::pin(workers.run(move || {
        executed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        drop(capture);
    }));
    assert!(poll_once(second.as_mut()).is_pending());
    drop(second);
    release.send(()).unwrap();
    first.await.unwrap();
    workers.run(|| ()).await.unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(executions.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancelled_running_job_releases_unreceived_output() {
    let workers = Workers::new(1).unwrap();
    let buffer = Arc::new(vec![7_u8; 4096]);
    let weak = Arc::downgrade(&buffer);
    let (started, running) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let mut task = Box::pin(workers.run(move || {
        started.send(()).unwrap();
        wait.recv().unwrap();
        buffer
    }));
    assert!(poll_once(task.as_mut()).is_pending());
    running.recv().unwrap();
    drop(task);
    release.send(()).unwrap();
    workers.run(|| ()).await.unwrap();
    assert!(weak.upgrade().is_none());
}
