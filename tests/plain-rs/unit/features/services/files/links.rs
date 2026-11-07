use super::*;
use std::future::Future;
use std::task::Poll;

#[test]
fn resource_resolution_yields_while_the_blocking_executor_is_busy() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let core = crate::test_runtime::Core::open(directory.path()).unwrap();
        let state = core.runtime_state();
        let (ready, started) = std::sync::mpsc::sync_channel(0);
        let (release, wait) = std::sync::mpsc::sync_channel(0);
        let occupied = tokio::task::spawn_blocking(move || {
            ready.send(()).unwrap();
            wait.recv().unwrap();
        });
        started.recv().unwrap();
        let resolution = resolve(&state, "invalid-resource", ReadKind::Original);
        tokio::pin!(resolution);
        let yielded = std::future::poll_fn(|context| {
            Poll::Ready(matches!(resolution.as_mut().poll(context), Poll::Pending))
        })
        .await;
        release.send(()).unwrap();
        occupied.await.unwrap();
        assert!(
            yielded,
            "resource decoding and database access must use the blocking executor"
        );
        assert!(resolution.await.is_err());
        core.stop_core().await;
    });
}
