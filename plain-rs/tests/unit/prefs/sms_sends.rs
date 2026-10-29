use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn multipart_success_waits_for_all_distinct_parts_and_keeps_correlation() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Db::open(&directory.path().join("plain.db")).unwrap();
    register(&prefs, "send-a".into(), Some("request-a".into()), 2, 100).unwrap();
    register(&prefs, "send-b".into(), Some("request-b".into()), 1, 100).unwrap();
    assert!(
        record(&prefs, "send-a", 0, 3, true, -1, 150)
            .unwrap()
            .is_none()
    );
    assert!(
        record(&prefs, "send-a", 2, 2, true, -1, 150)
            .unwrap()
            .is_none()
    );
    assert!(
        record(&prefs, "send-a", 0, 2, true, -1, 200)
            .unwrap()
            .is_none()
    );
    assert!(
        record(&prefs, "send-a", 0, 2, true, -1, 210)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        record(&prefs, "send-b", 0, 1, false, 4, 220).unwrap(),
        Some(Outcome {
            request_id: Some("request-b".into()),
            success: false,
            result_code: 4
        })
    );
    assert_eq!(
        record(&prefs, "send-a", 1, 2, true, -1, 300).unwrap(),
        Some(Outcome {
            request_id: Some("request-a".into()),
            success: true,
            result_code: -1
        })
    );
    assert!(
        record(&prefs, "send-a", 1, 2, false, 4, 301)
            .unwrap()
            .is_none()
    );
}

#[test]
fn partial_state_and_terminal_outbox_survive_reopen_and_expire_once() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plain.db");
    {
        let prefs = Db::open(&path).unwrap();
        register(&prefs, "send".into(), Some("request".into()), 2, 100).unwrap();
        record(&prefs, "send", 0, 2, true, -1, 200).unwrap();
    }
    let prefs = Db::open(&path).unwrap();
    let result = record(&prefs, "send", 1, 2, true, -1, 300)
        .unwrap()
        .unwrap();
    assert!(tick(&prefs, 300 + RETENTION_MS - 1).unwrap().is_empty());
    assert_eq!(pending(&prefs).unwrap()[0].result(), Some(result.clone()));
    drop(prefs);
    let prefs = Db::open(&path).unwrap();
    assert_eq!(pending(&prefs).unwrap()[0].result(), Some(result));
    assert!(tick(&prefs, 300 + RETENTION_MS).unwrap().is_empty());
    assert!(pending(&prefs).unwrap().is_empty());
    register(&prefs, "timeout".into(), None, 1, 100).unwrap();
    assert!(tick(&prefs, 100 + RETENTION_MS - 1).unwrap().is_empty());
    assert_eq!(
        tick(&prefs, 100 + RETENTION_MS).unwrap(),
        [Outcome {
            request_id: None,
            success: false,
            result_code: -1000
        }]
    );
    assert!(tick(&prefs, 100 + RETENTION_MS + 1).unwrap().is_empty());
    cancel(&prefs, "timeout").unwrap();
    assert!(pending(&prefs).unwrap().is_empty());
}

#[test]
fn concurrent_part_callbacks_commit_one_terminal_result() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Db::open(&directory.path().join("plain.db")).unwrap());
    register(&prefs, "send".into(), None, 16, 100).unwrap();
    let results = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for index in 0..16 {
            let prefs = &prefs;
            let results = &results;
            scope.spawn(move || {
                if record(prefs, "send", index, 16, true, -1, 200)
                    .unwrap()
                    .is_some()
                {
                    results.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    });
    assert_eq!(results.load(Ordering::SeqCst), 1);
    assert_eq!(pending(&prefs).unwrap()[0].completed_parts.len(), 16);
}
