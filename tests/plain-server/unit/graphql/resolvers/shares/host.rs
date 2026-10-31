use crate::server::ContentServer;
use async_graphql::Request;
use serde_json::{Value, json};
use std::sync::Arc;

#[tokio::test]
async fn main_share_content_uses_saved_roots_identity_and_live_port() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    let file = dir.path().join("shared.txt");
    std::fs::write(&file, b"abc").unwrap();
    let (_generation, mut requests) = server.runtime_state().host.connect();
    let executor = server.graphql_executor();
    let result = executor.execute(Request::new(format!(
        "mutation {{ createShare(name: \"A share\", realPaths: [{}], readOnly: true) {{ id }} }}", json!(file.to_str().unwrap())
    ))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    let id = data["createShare"]["id"].as_str().unwrap();
    let result = executor
        .execute(Request::new(format!(
            r#"mutation {{ sendShare(shareId:{}, targets:["local"]) {{content}} }}"#,
            json!(id)
        )))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    let message: Value =
        serde_json::from_str(data["sendShare"][0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(message["type"], "SHARE");
    assert_eq!(message["value"]["shareId"], id);
    assert_eq!(
        message["value"]["urlToken"],
        plain_rs::files::shares::Service::new(
            server.runtime_state().db.clone(),
            server.runtime_state().prefs.clone()
        )
        .token(id)
        .unwrap()
    );
    assert_eq!(message["value"]["name"], "A share");
    assert_eq!(message["value"]["itemCount"], 1);
    assert_eq!(message["value"]["totalSize"], 3);
    let state = server.runtime_state();
    assert_eq!(
        message["value"]["peerInfo"]["id"],
        state.native_client_id().unwrap()
    );
    assert_eq!(
        message["value"]["peerInfo"]["port"],
        state.public_ports().await.map(|ports| ports.1).unwrap_or(0)
    );
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn main_folder_share_and_batch_chat_use_shared_services_without_host_calls() {
    let directory = tempfile::tempdir().unwrap();
    let server = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap()),
    )
    .unwrap();
    let shared = directory.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::write(shared.join("file.txt"), b"abc").unwrap();
    let state = server.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let result = server.graphql_executor().execute(Request::new(r#"mutation($path:String!){sendFolderShare(targets:["local"],path:$path,name:"Folder"){id content status}}"#)
        .variables(async_graphql::Variables::from_json(json!({"path":shared})))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["sendFolderShare"][0]["status"], "SENT");
    let share = state.db.share_list().unwrap().pop().unwrap();
    let result = server.graphql_executor().execute(Request::new(r#"mutation($id:ID!){sendShare(targets:["local","peer:local"],shareId:$id){id content status}}"#)
        .variables(async_graphql::Variables::from_json(json!({"id":share.id})))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.data.into_json().unwrap()["sendShare"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let result = server.graphql_executor().execute(Request::new(r#"mutation($content:String!){sendChatItems(targets:["local","peer:local"],content:$content){id status}}"#)
        .variables(async_graphql::Variables::from_json(json!({"content":json!({"type":"TEXT","value":{"text":"Hello"}}).to_string()})))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.data.into_json().unwrap()["sendChatItems"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}
