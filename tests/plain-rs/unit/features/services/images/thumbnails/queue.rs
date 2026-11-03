use super::*;
use futures_util::poll;

fn job(key: impl Into<String>) -> Generation {
    Generation {
        request: Request {
            path: "/photo.jpg".into(),
            width: 256,
            height: 256,
            center_crop: true,
            file_id: String::new(),
            file_name: String::new(),
            access_order: None,
        },
        key: key.into(),
        cache: PathBuf::new(),
        cached: PathBuf::new(),
    }
}

#[tokio::test]
async fn newest_requests_run_first_and_reaccess_promotes_one_shared_job() {
    let queue = Queue::default();
    let mut first = Box::pin(queue.submit(job("first"), 1));
    let mut second = Box::pin(queue.submit(job("second"), 2));
    let mut third = Box::pin(queue.submit(job("third"), 3));
    assert!(poll!(&mut first).is_pending());
    assert!(poll!(&mut second).is_pending());
    assert!(poll!(&mut third).is_pending());
    assert_eq!(
        queue
            .pending
            .lock()
            .unwrap()
            .jobs
            .values()
            .rev()
            .map(|job| job.key.as_str())
            .collect::<Vec<_>>(),
        ["third", "second", "first"]
    );
    let mut reaccess = Box::pin(queue.submit(job("first"), 4));
    assert!(poll!(&mut reaccess).is_pending());
    let bytes = bytes::Bytes::from_static(b"shared image");
    {
        let mut pending = queue.pending.lock().unwrap();
        assert_eq!(
            pending
                .jobs
                .values()
                .rev()
                .map(|job| job.key.as_str())
                .collect::<Vec<_>>(),
            ["first", "third", "second"]
        );
        assert_eq!(pending.replies.len(), 3);
        for (_, entry) in pending.replies.drain() {
            entry
                .reply
                .send(Some(Arc::new(Ok(Some(bytes.clone())))))
                .unwrap();
        }
    }
    for result in [first.await, second.await, third.await, reaccess.await] {
        assert_eq!(result.unwrap().unwrap().as_ptr(), bytes.as_ptr());
    }
}

#[tokio::test]
async fn newest_access_keeps_all_2000_older_pending_requests() {
    let queue = Queue::default();
    let mut older = Vec::new();
    for index in 0..2000 {
        let mut job = Box::pin(queue.submit(job(format!("old-{index}")), index));
        assert!(poll!(&mut job).is_pending());
        older.push(job);
    }
    let mut newest = Box::pin(queue.submit(job("current-page"), 2000));
    assert!(poll!(&mut newest).is_pending());
    let pending = queue.pending.lock().unwrap();
    assert_eq!(pending.jobs.len(), 2001);
    assert_eq!(pending.jobs.last_key_value().unwrap().1.key, "current-page");
    assert_eq!(pending.replies.len(), 2001);
    for index in 0..2000 {
        assert!(pending.replies.contains_key(&format!("old-{index}")));
    }
}

#[tokio::test]
async fn delayed_old_file_lookups_cannot_overtake_a_new_page_or_demote_a_reaccess() {
    let queue = Queue::default();
    let mut newer = Box::pin(queue.submit(job("new-page"), 2));
    let mut delayed = Box::pin(queue.submit(job("old-page"), 1));
    assert!(poll!(&mut newer).is_pending());
    assert!(poll!(&mut delayed).is_pending());
    assert_eq!(
        queue
            .pending
            .lock()
            .unwrap()
            .jobs
            .last_key_value()
            .unwrap()
            .1
            .key,
        "new-page"
    );
    let mut reaccess = Box::pin(queue.submit(job("old-page"), 4));
    assert!(poll!(&mut reaccess).is_pending());
    let mut older_duplicate = Box::pin(queue.submit(job("old-page"), 0));
    assert!(poll!(&mut older_duplicate).is_pending());
    let pending = queue.pending.lock().unwrap();
    assert_eq!(pending.jobs.len(), 2);
    assert_eq!(pending.jobs.last_key_value().unwrap().1.key, "old-page");
    assert_eq!(*pending.jobs.last_key_value().unwrap().0, 4);
}

#[tokio::test]
async fn only_the_last_disconnected_subscriber_removes_a_pending_job() {
    let queue = Queue::default();
    let mut first = Box::pin(queue.submit(job("photo"), 1));
    let mut second = Box::pin(queue.submit(job("photo"), 2));
    assert!(poll!(&mut first).is_pending());
    assert!(poll!(&mut second).is_pending());
    drop(first);
    assert_eq!(queue.waiting(), 1);
    assert_eq!(queue.pending.lock().unwrap().jobs.len(), 1);
    drop(second);
    let pending = queue.pending.lock().unwrap();
    assert!(pending.jobs.is_empty());
    assert!(pending.replies.is_empty());
}

#[tokio::test]
async fn disconnecting_an_active_job_keeps_it_available_for_reaccess() {
    let queue = Queue::default();
    let mut first = Box::pin(queue.submit(job("photo"), 1));
    assert!(poll!(&mut first).is_pending());
    {
        let mut pending = queue.pending.lock().unwrap();
        pending.jobs.pop_last().unwrap();
        pending.replies.get_mut("photo").unwrap().queued_order = None;
    }
    drop(first);
    let mut reaccess = Box::pin(queue.submit(job("photo"), 2));
    assert!(poll!(&mut reaccess).is_pending());
    let mut pending = queue.pending.lock().unwrap();
    assert!(pending.jobs.is_empty());
    pending
        .replies
        .remove("photo")
        .unwrap()
        .reply
        .send(Some(Arc::new(Ok(None))))
        .unwrap();
    drop(pending);
    assert!(reaccess.await.unwrap().is_none());
}

#[tokio::test]
async fn dropping_an_old_reply_does_not_remove_a_new_job_for_the_same_key() {
    let queue = Queue::default();
    let mut old = Box::pin(queue.submit(job("photo"), 1));
    assert!(poll!(&mut old).is_pending());
    {
        let mut pending = queue.pending.lock().unwrap();
        pending.jobs.pop_last().unwrap();
        pending
            .replies
            .remove("photo")
            .unwrap()
            .reply
            .send(Some(Arc::new(Ok(None))))
            .unwrap();
    }
    let mut new = Box::pin(queue.submit(job("photo"), 2));
    assert!(poll!(&mut new).is_pending());
    drop(old);
    assert_eq!(queue.waiting(), 1);
    assert_eq!(queue.pending.lock().unwrap().jobs.len(), 1);
    drop(new);
    assert!(queue.pending.lock().unwrap().jobs.is_empty());
}
