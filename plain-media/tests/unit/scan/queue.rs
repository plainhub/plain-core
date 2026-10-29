use super::super::*;

#[test]
fn last_directory_completion_releases_all_waiting_workers() {
    for iteration in 0..1000 {
        let queue = Arc::new(ScanQueue::default());
        queue.push("root".into());
        let start = Arc::new(std::sync::Barrier::new(8));
        let (finished, results) = std::sync::mpsc::channel();
        let mut workers = Vec::new();
        for _ in 0..8 {
            let queue = queue.clone();
            let start = start.clone();
            let finished = finished.clone();
            workers.push(std::thread::spawn(move || {
                start.wait();
                while queue.pop().is_some() {
                    std::thread::yield_now();
                    queue.dir_done();
                }
                finished.send(()).unwrap();
            }));
        }
        drop(finished);
        let blocked = (0..8).any(|_| results.recv_timeout(Duration::from_secs(2)).is_err());
        let remaining = queue.active.load(Ordering::SeqCst);
        if blocked {
            let _guard = queue.stack.lock().unwrap();
            queue.cv.notify_all();
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(remaining, 0);
        assert!(
            !blocked,
            "directory completion left a worker asleep in iteration {iteration}"
        );
    }
}
