use super::*;
use futures_util::poll;
use std::task::Poll;

fn request() -> Request {
    Request {
        path: std::env::temp_dir()
            .join("preview.jpg")
            .to_string_lossy()
            .into_owned(),
        file_id: String::new(),
        file_name: "preview.jpg".into(),
        width: 256,
        height: 256,
        center_crop: true,
    }
}

#[tokio::test]
async fn foreground_is_dispatched_before_older_background_work() {
    let (queue, mut receivers) = Queue::new();
    let mut background = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "background".into(),
        PathBuf::new(),
        true,
    ));
    let mut foreground = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "foreground".into(),
        PathBuf::new(),
        false,
    ));
    assert!(poll!(&mut background).is_pending());
    assert!(poll!(&mut foreground).is_pending());
    assert!(queue.replies.lock().unwrap().contains_key("foreground"));
    assert_eq!(
        receivers
            .recv()
            .await
            .unwrap()
            .job
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .key,
        "foreground"
    );
    assert_eq!(
        receivers
            .recv()
            .await
            .unwrap()
            .job
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .key,
        "background"
    );
}

#[tokio::test]
async fn foreground_promotes_the_same_job_and_shares_its_result() {
    let (queue, mut receivers) = Queue::new();
    let mut background = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "shared".into(),
        PathBuf::new(),
        true,
    ));
    let mut foreground = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "shared".into(),
        PathBuf::new(),
        false,
    ));
    assert!(poll!(&mut background).is_pending());
    assert!(poll!(&mut foreground).is_pending());
    assert_eq!(queue.waiting(), 2);
    let promoted = receivers.recv().await.unwrap();
    let original = receivers.recv().await.unwrap();
    assert!(Arc::ptr_eq(&promoted, &original));
    assert!(promoted.job.lock().unwrap().take().is_some());
    assert!(original.job.lock().unwrap().take().is_none());
    let bytes = bytes::Bytes::from_static(b"same output");
    promoted
        .reply
        .send(Some(Arc::new(Ok(Some(bytes.clone())))))
        .unwrap();
    assert_eq!(background.await.unwrap().unwrap(), bytes);
    assert_eq!(foreground.await.unwrap().unwrap(), bytes);
}

#[tokio::test]
async fn full_background_lane_does_not_block_foreground_submission() {
    let (queue, mut receivers) = Queue::new();
    let mut background = Vec::new();
    for index in 0..320 {
        tokio::task::yield_now().await;
        let mut job = Box::pin(queue.submit(
            request(),
            PathBuf::new(),
            index.to_string(),
            PathBuf::new(),
            true,
        ));
        assert!(poll!(&mut job).is_pending());
        background.push(job);
    }
    assert_eq!(receivers.background.len(), 320);
    tokio::task::yield_now().await;
    let mut blocked = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "blocked".into(),
        PathBuf::new(),
        true,
    ));
    assert!(poll!(&mut blocked).is_pending());
    assert!(!queue.replies.lock().unwrap().contains_key("blocked"));
    let mut foreground = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "foreground".into(),
        PathBuf::new(),
        false,
    ));
    assert!(poll!(&mut foreground).is_pending());
    assert!(queue.replies.lock().unwrap().contains_key("foreground"));
    assert_eq!(
        receivers
            .recv()
            .await
            .unwrap()
            .job
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .key,
        "foreground"
    );
    drop(blocked);
    assert_eq!(queue.replies.lock().unwrap().len(), 321);
    drop(receivers);
    drop(background);
    drop(foreground);
    assert_eq!(queue.waiting(), 0);
}

#[tokio::test]
async fn closing_the_queue_releases_a_blocked_sender() {
    let (queue, receivers) = Queue::new();
    drop(receivers);
    let mut send = Box::pin(queue.submit(
        request(),
        PathBuf::new(),
        "closed".into(),
        PathBuf::new(),
        true,
    ));
    assert!(matches!(poll!(&mut send), Poll::Ready(Err(_))));
    assert!(queue.replies.lock().unwrap().is_empty());
}
