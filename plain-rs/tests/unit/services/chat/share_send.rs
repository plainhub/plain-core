use super::*;
use crate::test_runtime::Core as ContentServer;
use plain_rs::chat::enums::ChatStatus;

fn server(directory: &std::path::Path) -> ContentServer {
    ContentServer::start(
        directory,
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        Arc::new(plain_rs::prefs::Prefs::load(&directory.join("prefs.json")).unwrap()),
    )
    .unwrap()
}

#[tokio::test]
async fn folder_share_is_created_once_and_same_card_is_sent_to_every_target() {
    let directory = tempfile::tempdir().unwrap();
    let server = server(directory.path());
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let shared = directory.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::write(shared.join("a.txt"), b"abc").unwrap();
    std::fs::write(shared.join("b.txt"), b"de").unwrap();
    let (_generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    let messages = folder(
        &state,
        service,
        vec!["local".into(), "peer:local".into()],
        shared.to_string_lossy().into_owned(),
        "Files".into(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(messages.len(), 2);
    assert_ne!(messages[0].id, messages[1].id);
    assert!(
        messages
            .iter()
            .all(|message| message.status == ChatStatus::Sent)
    );
    assert_eq!(messages[0].content, messages[1].content);
    let shares = state.db.share_list().unwrap();
    assert_eq!(shares.len(), 1);
    assert!(shares[0].read_only);
    let content: serde_json::Value = serde_json::from_str(&messages[0].content).unwrap();
    assert_eq!(content["type"], "SHARE");
    assert_eq!(content["value"]["shareId"], shares[0].id);
    assert_eq!(content["value"]["itemCount"], 2);
    assert_eq!(content["value"]["totalSize"], 5);
    let mut content_changes = 0;
    let mut creations = 0;
    while let Ok(event) = events.try_recv() {
        content_changes += usize::from(event.event_type == "CONTENT_CHANGED");
        creations += usize::from(event.event_type == plain_rs::chat::events::WS_MESSAGE_CREATED);
    }
    assert_eq!(content_changes, 1);
    assert_eq!(creations, 2);
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn invalid_targets_and_nonfolders_cannot_leave_an_orphan_share() {
    let directory = tempfile::tempdir().unwrap();
    let server = server(directory.path());
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    for targets in [vec!["channel:".into()], vec!["local".into(); 129]] {
        assert!(
            folder(
                &state,
                service.clone(),
                targets,
                directory.path().to_string_lossy().into_owned(),
                "Invalid".into(),
                None
            )
            .await
            .is_err()
        );
    }
    let file = directory.path().join("file.txt");
    std::fs::write(&file, b"file").unwrap();
    assert!(
        folder(
            &state,
            service.clone(),
            vec!["local".into()],
            file.to_string_lossy().into_owned(),
            "File".into(),
            None
        )
        .await
        .is_err()
    );
    assert!(
        folder(
            &state,
            service.clone(),
            vec!["local".into()],
            directory.path().to_string_lossy().into_owned(),
            "Expired".into(),
            Some("2000-01-01T00:00:00Z".into())
        )
        .await
        .is_err()
    );
    assert!(
        folder(&state, service, vec![], String::new(), String::new(), None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        plain_rs::services::chat::chat_actions::send_many(
            &state,
            vec![],
            "unused invalid content".into()
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert!(state.db.share_list().unwrap().is_empty());
    assert_eq!(
        state
            .db
            .with_conn(|connection| connection
                .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0)))
            .unwrap(),
        0
    );
    server.shutdown().await;
}

#[tokio::test]
async fn failed_remote_delivery_retains_share_and_successful_local_message() {
    let directory = tempfile::tempdir().unwrap();
    let server = server(directory.path());
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let shared = directory.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    let _ = folder(
        &state,
        service,
        vec!["local".into(), "peer:missing".into()],
        shared.to_string_lossy().into_owned(),
        "Files".into(),
        None,
    )
    .await;
    assert_eq!(state.db.share_list().unwrap().len(), 1);
    let rows = state
        .db
        .with_conn(|connection| {
            connection
                .prepare("SELECT to_id,status FROM chats ORDER BY to_id")?
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|(target, status)| target == "local" && status == "SENT")
    );
    assert!(
        rows.iter()
            .any(|(target, status)| target == "missing" && status != "SENT")
    );
    server.shutdown().await;
}

#[tokio::test]
async fn folder_share_and_all_messages_roll_back_when_a_later_message_insert_fails() {
    let directory = tempfile::tempdir().unwrap();
    let core = server(directory.path());
    let state = core.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let folder_path = directory.path().join("owned-folder");
    std::fs::create_dir(&folder_path).unwrap();
    std::fs::write(folder_path.join("owned.txt"), b"owned bytes").unwrap();
    let existing = service
        .create(
            "Existing share".into(),
            vec![folder_path.to_string_lossy().into_owned()],
            true,
            None,
        )
        .unwrap();
    state.db.with_conn(|connection| connection.execute_batch(
        "CREATE TRIGGER reject_second_share_message BEFORE INSERT ON chats WHEN (SELECT COUNT(*) FROM chats)>0 BEGIN SELECT RAISE(ABORT,'owned batch failure'); END;"
    )).unwrap();
    let mut events = state.events.subscribe();
    let result = folder(
        &state,
        service,
        vec!["local".into(), "peer:local".into()],
        folder_path.to_string_lossy().into_owned(),
        "Owned folder".into(),
        None,
    )
    .await;
    assert!(result.is_err());
    let shares = state.db.share_list().unwrap();
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].id, existing.id);
    assert_eq!(
        state
            .db
            .with_conn(|connection| connection
                .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0)))
            .unwrap(),
        0
    );
    assert_eq!(
        std::fs::read(folder_path.join("owned.txt")).unwrap(),
        b"owned bytes"
    );
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CONTENT_CHANGED");
        assert_ne!(event.event_type, plain_rs::chat::events::WS_MESSAGE_CREATED);
    }
    core.shutdown().await;
}

#[tokio::test]
async fn a_committed_folder_share_survives_a_later_delivery_error() {
    let directory = tempfile::tempdir().unwrap();
    let core = server(directory.path());
    let state = core.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    state
        .prefs
        .set(
            plain_rs::prefs::pref_keys::SIGNATURE_KEY_PAIR,
            "invalid signing fixture",
        )
        .unwrap();
    let folder_path = directory.path().join("owned-folder");
    std::fs::create_dir(&folder_path).unwrap();
    let result = folder(
        &state,
        service,
        vec!["peer:remote".into()],
        folder_path.to_string_lossy().into_owned(),
        "Owned folder".into(),
        None,
    )
    .await;
    assert!(result.is_err());
    let shares = state.db.share_list().unwrap();
    assert_eq!(shares.len(), 1);
    let content = state
        .db
        .with_conn(|connection| {
            connection.query_row("SELECT content FROM chats", [], |row| {
                row.get::<_, String>(0)
            })
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&content).unwrap()["value"]["shareId"],
        shares[0].id
    );
    assert!(folder_path.exists());
    core.shutdown().await;
}
