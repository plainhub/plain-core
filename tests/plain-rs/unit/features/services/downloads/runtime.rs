use crate::runtime::Runtime as CoreRuntime;
use plain_libs::serde_json::json;
use std::sync::Arc;

fn fixture() -> (
    plain_libs::temporary::TempDir,
    crate::test_runtime::Core,
    CoreRuntime,
) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let peer = crate::infrastructure::db::DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    crate::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[peer],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    (dir, server, state)
}
fn seed_chat(state: &CoreRuntime, id: &str, size: usize) {
    let row=crate::infrastructure::db::DChat::new("peer","me","",&json!({"type":"FILES","value":{"items":[{"id":id,"uri":format!("fsid:{id}"),"fileName":"fixture.bin","size":size}]}}).to_string());
    let mut row = row;
    row.id = id.into();
    state.db.insert_chat(&row).unwrap();
}
#[tokio::test]
async fn paused_generation_and_shutdown_reject_late_bytes_and_release_transfer_files() {
    let (dir, server, state) = fixture();
    seed_chat(&state, "slow", 8192);
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let download = crate::features::services::downloads::create(
        &state,
        crate::features::downloads::Source::Chat {
            message_id: "slow".into(),
            peer_id: "peer".into(),
            file_ids: vec!["slow".into()],
        },
        None,
        None,
    )
    .await
    .unwrap();
    let caps = requests.recv().await.unwrap();
    state
        .host
        .reply(generation, json!({"id":caps["id"],"result":["BLE"]}))
        .unwrap();
    let old = requests.recv().await.unwrap();
    state
        .download_runtime
        .control(
            &state,
            &download.id,
            crate::features::downloads::Command::Pause,
        )
        .await
        .unwrap();
    assert_eq!(
        state.downloads.snapshot()[0].status,
        crate::features::downloads::Status::Paused
    );
    assert_eq!(
        std::fs::read_dir(dir.path().join("attachment-transfers"))
            .unwrap()
            .count(),
        0
    );
    let _late_reply = state.host.reply(generation,json!({"id":old["id"],"result":json!({"s":200,"b":plain_libs::utils::base64::base64_encode(&vec![1;8192])}).to_string()}));
    assert_eq!(state.downloads.snapshot()[0].downloaded_bytes, 0);
    state
        .download_runtime
        .control(
            &state,
            &download.id,
            crate::features::downloads::Command::Resume,
        )
        .await
        .unwrap();
    let caps = requests.recv().await.unwrap();
    state
        .host
        .reply(generation, json!({"id":caps["id"],"result":["BLE"]}))
        .unwrap();
    let _exchange = requests.recv().await.unwrap();
    state.host.disconnect(generation);
    server.shutdown().await;
    assert!(state.transport.active().is_empty());
    assert_eq!(
        std::fs::read_dir(dir.path().join("attachment-transfers"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn queue_changes_refresh_the_host_through_the_unified_callback() {
    let (_directory, core, state) = fixture();
    let (generation, mut requests) = state.host.connect();
    state.downloads.stop();
    let request = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let request = requests.recv().await.unwrap();
            state
                .host
                .reply(generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "downloadsRefresh" {
                break request;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(request["params"], json!({}));
    state.host.disconnect(generation);
    core.shutdown().await;
}
