use super::*;
use crate::infrastructure::db::{
    DChat, DPeer,
    chat_store::{self, SaveMode},
};
use plain_libs::serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn both_attachment_kinds_recheck_message_peer_and_uri_before_execution() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let peer = DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    chat_store::peers::save(&state.db, &[peer], SaveMode::Insert).unwrap();
    for kind in ["FILES", "IMAGES"] {
        let row = DChat::new("peer", "me", "", &json!({"type": kind, "value": {"items": [{"id": "file", "uri": "fsid:remote", "fileName": "fixture.bin", "size": 3}]}}).to_string());
        state.db.insert_chat(&row).unwrap();
        assert_eq!(
            files(&state, &row.id, "peer", &["file".into()], &HashMap::new())
                .unwrap()
                .len(),
            1
        );
        assert!(
            files(
                &state,
                &row.id,
                "peer",
                &["missing".into()],
                &HashMap::new()
            )
            .is_err()
        );
        for file_ids in [vec![], vec!["file".into(), "file".into()]] {
            assert!(
                super::super::create(
                    &state,
                    Source::Chat {
                        message_id: row.id.clone(),
                        peer_id: "peer".into(),
                        file_ids
                    },
                    None,
                    None
                )
                .await
                .is_err()
            );
        }
        let changed = DChat { content: json!({"type":kind,"value":{"items":[{"id":"file","uri":"fid:local","fileName":"fixture.bin","size":3}]}}).to_string(), ..row.clone() };
        chat_store::messages::save(&state.db, &[changed], SaveMode::Update).unwrap();
        assert!(files(&state, &row.id, "peer", &["file".into()], &HashMap::new()).is_err());
    }
    assert!(state.downloads.snapshot().is_empty());
    server.shutdown().await;
}

#[tokio::test]
async fn incoming_files_and_images_preserve_independent_attachment_tasks() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let peer = DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    chat_store::peers::save(&state.db, &[peer], SaveMode::Insert).unwrap();
    for kind in ["FILES", "IMAGES"] {
        let row = DChat::new("peer", "me", "", &json!({"type":kind,"value":{"items":[{"id":"first","uri":"fsid:one","fileName":"one.bin","size":1},{"id":"second","uri":"fsid:two","fileName":"two.bin","size":2}]}}).to_string());
        state.db.insert_chat(&row).unwrap();
        received(&state, &row, "peer").await.unwrap();
        received(&state, &row, "peer").await.unwrap();
        let tasks: Vec<_> = state.downloads.snapshot().into_iter().filter(|task| matches!(&task.source, Source::Chat { message_id, .. } if message_id == &row.id)).collect();
        assert_eq!(tasks.len(), 2);
        assert!(
            super::super::create(
                &state,
                Source::Chat {
                    message_id: row.id.clone(),
                    peer_id: "peer".into(),
                    file_ids: vec!["first".into(), "first".into()]
                },
                None,
                None
            )
            .await
            .is_err()
        );
        for file_id in ["first", "second"] {
            let task = tasks.iter().find(|task| matches!(&task.source, Source::Chat { file_ids, .. } if file_ids == &[file_id.to_string()])).unwrap();
            state
                .download_runtime
                .control(
                    &state,
                    &task.id,
                    crate::features::downloads::Command::Remove,
                )
                .await
                .unwrap();
        }
    }
    core.shutdown().await;
}

#[tokio::test]
async fn queued_chat_sources_cannot_be_replaced_before_execution() {
    for kind in ["FILES", "IMAGES"] {
        for changed in ["uri", "size", "peer"] {
            let directory = plain_libs::temporary::tempdir().unwrap();
            let core = crate::test_runtime::Core::open(directory.path()).unwrap();
            let state = core.runtime_state();
            let mut peer = DPeer::new(
                "peer",
                "fixture",
                "",
                Some(443),
                crate::features::chat::enums::DeviceType::Phone,
            );
            peer.public_key = "original".into();
            chat_store::peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
            let mut content = json!({"type":kind,"value":{"items":[{"id":"file","uri":"fsid:original","fileName":"fixture.bin","size":3}]}});
            let mut row = DChat::new("peer", "me", "", &content.to_string());
            state.db.insert_chat(&row).unwrap();
            let mut creation = Box::pin(super::super::create(
                &state,
                Source::Chat {
                    message_id: row.id.clone(),
                    peer_id: peer.id.clone(),
                    file_ids: vec!["file".into()],
                },
                None,
                None,
            ));
            let task = match futures_util::poll!(creation.as_mut()) {
                std::task::Poll::Ready(result) => result.unwrap(),
                std::task::Poll::Pending => panic!("Chat creation must not begin transport IO"),
            };
            match changed {
                "peer" => {
                    peer.public_key = "replacement".into();
                    chat_store::peers::save(&state.db, &[peer], SaveMode::Update).unwrap();
                }
                field => {
                    content["value"]["items"][0][field] = if field == "uri" {
                        json!("fsid:replacement")
                    } else {
                        json!(4)
                    };
                    row.content = content.to_string();
                    chat_store::messages::save(&state.db, &[row], SaveMode::Update).unwrap();
                }
            }
            let mut changed = state.downloads.changed.subscribe();
            let completed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let current = state
                        .downloads
                        .snapshot()
                        .into_iter()
                        .find(|current| current.id == task.id)
                        .unwrap();
                    if current.status.terminal() {
                        return current;
                    }
                    changed.changed().await.unwrap();
                }
            })
            .await
            .unwrap();
            assert_eq!(completed.status, crate::features::downloads::Status::Failed);
            assert_eq!((completed.done_files, completed.downloaded_bytes), (0, 0));
            assert!(completed.error.contains("changed"), "{}", completed.error);
            let transfers = directory.path().join("attachment-transfers");
            if transfers.exists() {
                assert_eq!(std::fs::read_dir(transfers).unwrap().count(), 0);
            }
            core.shutdown().await;
        }
    }
}
