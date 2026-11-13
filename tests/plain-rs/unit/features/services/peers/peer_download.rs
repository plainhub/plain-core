use super::*;
use crate::features::chat::attachments::attachment_imports::Imports;
use crate::features::chat::attachments::attachment_imports::Ticket;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

fn fixture() -> (
    plain_libs::temporary::TempDir,
    crate::infrastructure::db::Db,
    Arc<Imports>,
    Ticket,
) {
    let root = plain_libs::temporary::tempdir().unwrap();
    let db = crate::infrastructure::db::Db::open(&root.path().join("plain.db")).unwrap();
    let peer = crate::infrastructure::db::DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    crate::infrastructure::db::chat_store::peers::save(
        &db,
        &[peer],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let mut chat = crate::infrastructure::db::DChat::new("peer", "me", "", &json!({
        "type":"FILES", "value":{"items":[{"id":"file","uri":"fsid:remote-file","fileName":"fixture.bin","size":7}]}
    }).to_string());
    chat.id = "message".into();
    db.insert_chat(&chat).unwrap();
    let imports = Arc::new(Imports::default());
    let ticket = imports
        .begin(&db, root.path(), "message", "file", "fsid:remote-file")
        .unwrap();
    (root, db, imports, ticket)
}

#[test]
fn canceled_queued_ticket_open_cannot_recreate_the_unlinked_import_file() {
    for action in ["pause", "cancel", "remove"] {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        let (root, _db, imports, ticket) = fixture();
        let path = std::path::PathBuf::from(&ticket.path);
        let neighbor = root.path().join("neighbor.bin");
        std::fs::write(&neighbor, b"preserved neighbor").unwrap();
        assert!(path.exists());
        runtime.block_on(async {
            let (started, ready) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                started.send(()).unwrap();
                blocked.recv().unwrap();
            });
            ready.await.unwrap();
            let mut opening = Box::pin(open_ticket(&ticket.path));
            let queued = futures_util::poll!(opening.as_mut()).is_pending();
            let canceled = imports.abort(&ticket.token).unwrap();
            let removed_before_open = !path.exists();
            drop(opening);
            release.send(()).unwrap();
            blocker.await.unwrap();
            tokio::task::spawn_blocking(|| {}).await.unwrap();
            assert!(queued);
            assert!(canceled);
            assert!(removed_before_open);
        });
        runtime.shutdown_timeout(Duration::from_secs(5));
        assert!(
            !path.exists(),
            "Canceled {action} ticket was recreated by a late open"
        );
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            0
        );
        assert_eq!(std::fs::read(neighbor).unwrap(), b"preserved neighbor");
    }
}

#[tokio::test]
async fn existing_ticket_is_truncated_and_exact_download_bytes_commit_normally() {
    let (root, db, imports, ticket) = fixture();
    std::fs::write(&ticket.path, b"stale bytes must be truncated").unwrap();
    let payload = [0, 255, 128, 10, 13, 42, 1];
    let mut file = open_ticket(&ticket.path).await.unwrap();
    file.write_all(&payload).await.unwrap();
    file.flush().await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);
    assert_eq!(std::fs::read(&ticket.path).unwrap(), payload);
    imports.finish(&db, root.path(), &ticket.token).unwrap();
    assert!(!std::path::Path::new(&ticket.path).exists());
    let chat = crate::infrastructure::db::chat_store::messages::get(&db, "message")
        .unwrap()
        .unwrap();
    let content: Value = plain_libs::serde_json::from_str(&chat.content).unwrap();
    let uri = content["value"]["items"][0]["uri"].as_str().unwrap();
    let hash = uri.strip_prefix("fid:").unwrap().split('.').next().unwrap();
    let stored = db.app_file_get(hash).unwrap().unwrap();
    assert_eq!(
        std::fs::read(root.path().join(stored.real_path)).unwrap(),
        payload
    );
}

#[tokio::test]
async fn missing_ticket_open_fails_without_creating_a_new_file() {
    let (_root, _db, imports, ticket) = fixture();
    assert!(imports.abort(&ticket.token).unwrap());
    let error = open_ticket(&ticket.path).await.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(!std::path::Path::new(&ticket.path).exists());
}

#[test]
fn terminal_failure_aborts_its_ticket_but_an_active_import_legitimately_has_a_file() {
    let (_root, db, imports, ticket) = fixture();
    assert!(std::path::Path::new(&ticket.path).is_file());
    assert!(imports.abort(&ticket.token).unwrap());
    assert!(!std::path::Path::new(&ticket.path).exists());
    let chat = crate::infrastructure::db::chat_store::messages::get(&db, "message")
        .unwrap()
        .unwrap();
    let content: Value = plain_libs::serde_json::from_str(&chat.content).unwrap();
    assert_eq!(content["value"]["items"][0]["uri"], "fsid:remote-file");
}
