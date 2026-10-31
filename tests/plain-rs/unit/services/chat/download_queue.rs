use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::json;
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, crate::test_runtime::Core, CoreRuntime) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let peer = plain_rs::db::DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    (dir, server, state)
}
fn seed_chat(state: &CoreRuntime, id: &str, size: usize) {
    let row=plain_rs::db::DChat::new("peer","me","",&json!({"type":"FILES","value":{"items":[{"id":id,"uri":format!("fsid:{id}"),"fileName":"fixture.bin","size":size}]}}).to_string());
    let mut row = row;
    row.id = id.into();
    state.db.insert_chat(&row);
}
#[tokio::test]
async fn paused_generation_and_shutdown_reject_late_bytes_and_release_transfer_files() {
    let (dir, server, state) = fixture();
    seed_chat(&state, "slow", 8192);
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    state.downloads.enqueue("slow", "slow", "peer").unwrap();
    let caps = requests.recv().await.unwrap();
    state
        .host
        .reply(generation, json!({"id":caps["id"],"result":["BLE"]}))
        .unwrap();
    let old = requests.recv().await.unwrap();
    let row = state.downloads.snapshot()["tasks"][0].clone();
    state.downloads.control("slow", "pause").unwrap();
    state.host.reply(generation,json!({"id":old["id"],"result":json!({"s":200,"b":plain_rs::utils::base64::base64_encode(&vec![1;8192])}).to_string()})).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(state.downloads.snapshot()["tasks"][0]["status"], "PAUSED");
    assert!(
        !state
            .downloads
            .finish("slow", row["generation"].as_str().unwrap(), None)
            .unwrap()
    );
    state.downloads.control("slow", "resume").unwrap();
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
