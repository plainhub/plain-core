use super::*;

#[tokio::test]
async fn cancelled_running_job_keeps_admission_until_its_buffers_are_released() {
    let admission = Admission::new(1, 1);
    let (started, running) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let buffer = std::sync::Arc::new(vec![1_u8; 4096]);
    let weak = std::sync::Arc::downgrade(&buffer);
    let mut task = Box::pin(admission.run(4096, move || {
        assert!(
            std::thread::current()
                .name()
                .unwrap()
                .starts_with("thumbnail-")
        );
        started.send(()).unwrap();
        wait.recv().unwrap();
        drop(buffer);
    }));
    assert!(poll_once(task.as_mut()).is_pending());
    running.recv().unwrap();
    drop(task);
    assert_eq!(admission.cpu.available_permits(), 0);
    assert_eq!(admission.mem.available_permits(), 15);
    let mut next = Box::pin(admission.run(4096, move || weak.upgrade().is_none()));
    assert!(poll_once(next.as_mut()).is_pending());
    release.send(()).unwrap();
    assert!(next.await.unwrap());
    assert_eq!(admission.cpu.available_permits(), 1);
    assert_eq!(admission.mem.available_permits(), 16);
}

#[tokio::test]
async fn rejected_job_never_initializes_workers_or_runs() {
    let admission = Admission::new(1, 1);
    assert!(
        admission
            .run(2 * 1024 * 1024, || panic!("over budget"))
            .await
            .is_err()
    );
    assert!(admission.pool.get().is_none());
}

#[test]
fn memory_charges_round_up() {
    assert_eq!(units_for(0), 0);
    assert_eq!(units_for(MEM_UNIT - 1), 1);
    assert_eq!(units_for(MEM_UNIT), 1);
    assert_eq!(units_for(MEM_UNIT * 2 + 1), 3);
}

#[test]
fn config_clamps_budget() {
    assert_eq!(build(1).mem_budget_mb(), 1);
    assert_eq!(build(999_999).mem_budget_mb(), 16_384);
    assert_eq!(build(2048).mem_budget_mb(), 2048);
    assert_eq!(build(0).mem_budget_mb(), 2048, "default budget");
}

#[tokio::test]
async fn small_jobs_consume_memory_and_release_it_on_drop() {
    let adm = Admission::new(64, 1);
    let mut held = Vec::new();
    for _ in 0..16 {
        held.push(adm.acquire(16 * 1024).await.unwrap());
    }
    let mut waiter = Box::pin(adm.acquire(1));
    assert!(poll_once(waiter.as_mut()).is_pending());
    held.pop();
    assert!(poll_once(waiter.as_mut()).is_ready());
}

fn poll_once<F: std::future::Future>(future: std::pin::Pin<&mut F>) -> std::task::Poll<F::Output> {
    future.poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

#[tokio::test]
async fn cpu_limit_applies_even_when_memory_is_free() {
    let adm = Admission::new(3, 128);
    let mut held = Vec::new();
    for _ in 0..adm.workers() {
        held.push(adm.acquire(0).await.unwrap());
    }
    let mut next = Box::pin(adm.acquire(0));
    assert!(poll_once(next.as_mut()).is_pending());
    held.pop();
    assert!(poll_once(next.as_mut()).is_ready());
}

#[tokio::test]
async fn cancelling_a_memory_waiter_releases_its_cpu_slot() {
    let adm = Admission::new(2, 1);
    let held = adm.acquire(1024 * 1024).await.unwrap();
    let mut waiter = Box::pin(adm.acquire(1));
    assert!(poll_once(waiter.as_mut()).is_pending());
    assert_eq!(adm.cpu.available_permits(), 0);
    drop(waiter);
    assert_eq!(adm.cpu.available_permits(), 1);
    drop(held);
    assert_eq!(adm.cpu.available_permits(), 2);
    assert_eq!(adm.mem.available_permits(), 16);
    assert!(adm.acquire(1024 * 1024 + 1).await.is_err());
}

#[tokio::test]
async fn oversize_single_job_rejected_fast() {
    let adm = Admission::new(4, 128); // 128 MB
    assert!(
        adm.acquire(300 * 1024 * 1024).await.is_err(),
        "300MB job must not be admitted into a 128MB budget"
    );
    // Exactly-fitting job is fine.
    assert!(adm.acquire(128 * 1024 * 1024).await.is_ok());
}

#[tokio::test]
async fn budget_blocks_until_released() {
    let adm = Admission::new(4, 1); // 1 MB
    let held = adm.acquire(1024 * 1024).await.unwrap();
    let waiter = adm.acquire(1024 * 1024);
    tokio::pin!(waiter);
    assert!(poll_once(waiter.as_mut()).is_pending());
    drop(held);
    assert!(poll_once(waiter.as_mut()).is_ready());
}
