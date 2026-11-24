use crate::server::ContentServer;
use plain_libs::serde_json::{Value, json};
use plain_rs::features::chat::enums::{DeviceType, PeerStatus};
use plain_rs::features::services::clipboard::report;
use plain_rs::features::sessions::SessionType;
use plain_rs::infrastructure::db::chat_store::{SaveMode, peers};
use plain_rs::infrastructure::db::{DPeer, SessionRow};
use plain_rs::runtime::Runtime;
use std::time::Duration;
use tokio::sync::mpsc;

async fn device(directory: &std::path::Path) -> (ContentServer, Runtime, u16) {
    let server =
        ContentServer::open_with_cache(&directory.join("data"), &directory.join("cache")).unwrap();
    let state = server.runtime_state().core;
    state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    state.prefs.set("http_port", 0).unwrap();
    state.prefs.set("https_port", 0).unwrap();
    let (_, port) = server.start_configured_public().await.unwrap();
    (server, state, port)
}

fn grant(receiver: &Runtime, caller: &str, kind: SessionType, seed: u8) -> SessionRow {
    let now = plain_libs::utils::dbtime::now_iso();
    let session = SessionRow {
        client_id: caller.into(),
        url_token: plain_libs::crypto::gen_token(),
        name: "Synthetic management fixture".into(),
        r#type: kind,
        client_ip: String::new(),
        os_name: String::new(),
        os_version: String::new(),
        app_name: String::new(),
        app_version: String::new(),
        token: plain_libs::utils::base64::base64_encode(&[seed; 32]),
        last_seen_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    receiver.db.session_save(&session).unwrap();
    session
}

fn peer(owner: &Runtime, grant: &SessionRow, remote_id: &str, remote_port: u16) {
    let mut peer = DPeer::new(
        remote_id,
        "Synthetic device",
        "127.0.0.1",
        Some(remote_port),
        DeviceType::Computer,
    );
    peer.status = PeerStatus::Paired;
    peer.token.clone_from(&grant.token);
    peers::save(&owner.db, &[peer], SaveMode::Insert).unwrap();
}

fn host(
    state: &Runtime,
) -> (
    mpsc::UnboundedReceiver<()>,
    mpsc::UnboundedReceiver<Value>,
    tokio::task::JoinHandle<()>,
) {
    let state = state.clone();
    let (generation, mut requests) = state.host.connect();
    let (accessed, access) = mpsc::unbounded_channel();
    let (wrote, writes) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let params = &request["params"];
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["CLIPBOARD"]),
                "systemAccessFacts" => {
                    if params["features"] == json!(["CLIPBOARD"]) {
                        let _ = accessed.send(());
                    }
                    let granted = params["features"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|feature| (feature.as_str().unwrap().to_owned(), json!(true)))
                        .collect::<plain_libs::serde_json::Map<_, _>>();
                    json!({"granted":granted})
                }
                "systemSetClipboard" => {
                    let _ = wrote.send(params.clone());
                    if params["text"] == "reject OS write" {
                        json!(false)
                    } else {
                        report(
                            &state,
                            &json!({"text":params["text"],"sensitive":params["sensitive"]})
                                .to_string(),
                        )
                        .unwrap();
                        json!(true)
                    }
                }
                _ => Value::Null,
            };
            let _ = state
                .host
                .reply(generation, json!({"id":request["id"],"result":result}));
        }
    });
    (access, writes, task)
}

async fn quiet(writes: &mut mpsc::UnboundedReceiver<Value>) {
    assert!(
        tokio::time::timeout(Duration::from_millis(350), writes.recv())
            .await
            .is_err()
    );
}

async fn write(writes: &mut mpsc::UnboundedReceiver<Value>, text: &str) {
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(8), writes.recv())
            .await
            .unwrap()
            .unwrap(),
        json!({"text":text,"label":"","sensitive":false})
    );
}

fn direction(state: &Runtime, peer_id: &str, value: &str) {
    state
        .prefs
        .set("clipboard_directions", json!({peer_id:value}))
        .unwrap();
    let _ = state
        .events
        .send(plain_rs::infrastructure::events::WsEvent::broadcast(
            "PREFS_UPDATED",
            String::new(),
        ));
}

fn capture(state: &Runtime, text: &str, sensitive: bool) {
    report(
        state,
        &json!({"text":text,"sensitive":sensitive}).to_string(),
    )
    .unwrap();
}

#[tokio::test]
async fn managed_encrypted_live_stream_syncs_only_when_both_local_directions_agree() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let (a, sender, sender_port) = device(&directory.path().join("sender")).await;
    let (b, receiver, receiver_port) = device(&directory.path().join("receiver")).await;
    let sender_id = sender.native_client_id().unwrap();
    let receiver_id = receiver.native_client_id().unwrap();
    let (mut sender_access, mut sender_writes, sender_host) = host(&sender);
    let (_, mut receiver_writes, receiver_host) = host(&receiver);

    let inbound = grant(&sender, &receiver_id, SessionType::Login, 31);
    let reverse = grant(&receiver, &sender_id, SessionType::Login, 32);
    direction(&sender, &receiver_id, "SEND");
    direction(&receiver, &sender_id, "BOTH");

    capture(&sender, "before connection", false);
    peer(&receiver, &inbound, &sender_id, sender_port);
    // Existing LOGIN by itself does not establish clipboard sync authority.
    tokio::time::timeout(Duration::from_secs(8), sender_access.recv())
        .await
        .unwrap()
        .unwrap();
    capture(&sender, "one-way management only", false);
    quiet(&mut receiver_writes).await;

    let mut chat_only = reverse.clone();
    chat_only.token.clear();
    peer(&sender, &chat_only, &receiver_id, receiver_port);
    capture(&sender, "chat only cannot receive", false);
    quiet(&mut receiver_writes).await;
    sender
        .db
        .with_conn(|db| {
            db.execute(
                "UPDATE peers SET status='UNPAIRED',manage_token=?1 WHERE id=?2",
                plain_libs::rusqlite::params![plain_libs::utils::base64::base64_decode(&reverse.token), receiver_id],
            )
        })
        .unwrap();
    capture(&sender, "unpaired with stale token cannot receive", false);
    quiet(&mut receiver_writes).await;
    sender
        .db
        .update_peer_status(&receiver_id, PeerStatus::Paired);
    quiet(&mut receiver_writes).await;
    capture(&sender, "live exact capture", false);
    write(&mut receiver_writes, "live exact capture").await;
    quiet(&mut receiver_writes).await;
    quiet(&mut sender_writes).await;
    let rows = receiver.db.clipboard_page("", 20, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "live exact capture");
    assert_eq!(rows[0].source, sender_id);

    direction(&receiver, &sender_id, "SEND");
    quiet(&mut receiver_writes).await; // allow the existing WS complete to arrive
    while sender_access.try_recv().is_ok() {}
    capture(&sender, "receive disabled", false);
    quiet(&mut receiver_writes).await;
    assert!(
        sender_access.try_recv().is_err(),
        "receiver did not retain an active payload stream"
    );
    direction(&receiver, &sender_id, "BOTH");
    tokio::time::timeout(Duration::from_secs(8), sender_access.recv())
        .await
        .unwrap()
        .unwrap();
    quiet(&mut receiver_writes).await; // no backfill on a fresh operation
    capture(&sender, "fresh after subscribe", false);
    write(&mut receiver_writes, "fresh after subscribe").await;

    sender
        .prefs
        .set(
            "clipboard_directions",
            json!({receiver_id.clone():"RECEIVE"}),
        )
        .unwrap();
    capture(&sender, "send disabled", false);
    quiet(&mut receiver_writes).await;
    direction(&sender, &receiver_id, "SEND");
    capture(&sender, "private capture", true);
    quiet(&mut receiver_writes).await;
    capture(&sender, "reject OS write", false);
    write(&mut receiver_writes, "reject OS write").await;
    quiet(&mut receiver_writes).await;
    assert_eq!(receiver.db.clipboard_count_rows().unwrap(), 2);

    direction(&sender, &receiver_id, "OFF");
    let explicit = plain_rs::features::services::peers::management_http::query(
        &receiver,
        &plain_libs::protocols::http_client::Url::parse(&format!("https://127.0.0.1:{sender_port}")).unwrap(),
        &receiver_id,
        &plain_libs::utils::base64::base64_decode(&inbound.token),
        json!({"query":r#"mutation {setClipboard(text:"explicit manual write",label:"Manual",sensitive:true)}"#}),
        65536,
    ).await.unwrap();
    assert_eq!(explicit["setClipboard"], true);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(8), sender_writes.recv())
            .await
            .unwrap()
            .unwrap(),
        json!({"text":"explicit manual write","label":"Manual","sensitive":true})
    );
    let explicit_row = sender
        .db
        .clipboard_page("explicit manual write", 1, 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(explicit_row.source, receiver_id);
    assert_eq!(explicit_row.label, "Manual");
    assert!(explicit_row.sensitive);
    quiet(&mut receiver_writes).await;

    direction(&sender, &receiver_id, "SEND");
    let mut invalid = inbound.clone();
    invalid.r#type = SessionType::ApiToken;
    sender.db.session_save(&invalid).unwrap();
    capture(
        &sender,
        "API token cannot receive automatic clipboard",
        false,
    );
    quiet(&mut receiver_writes).await;
    let mut rotated = inbound.clone();
    rotated.token = plain_libs::utils::base64::base64_encode(&[33; 32]);
    sender.db.session_save(&rotated).unwrap();
    capture(
        &sender,
        "stale authenticated socket after rotation cannot receive",
        false,
    );
    quiet(&mut receiver_writes).await;
    sender.db.session_delete(&receiver_id).unwrap();
    capture(
        &sender,
        "revoked login cannot receive automatic clipboard",
        false,
    );
    quiet(&mut receiver_writes).await;

    b.stop_core().await;
    a.stop_core().await;
    receiver_host.abort();
    sender_host.abort();
}
