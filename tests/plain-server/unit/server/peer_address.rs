use plain_libs::protocols::http_client;
use plain_libs::serde_json::json;
use plain_rs::infrastructure::db::DPeer;
#[tokio::test]
async fn main_peer_records_and_file_urls_use_current_addresses_without_host() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let mut peer = DPeer::new(
        "fixture",
        " ",
        "127.0.0.1",
        Some(443),
        plain_rs::features::chat::enums::DeviceType::Tablet,
    );
    plain_rs::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        plain_rs::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let native = server.native_graphql().unwrap();
    let response = native
        .execute(r#"{fixture:peers{name ip} peers{name ip}}"#.into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let rows = response.data.into_json().unwrap();
    let row = &rows["fixture"][0];
    assert_eq!(row["name"], " ");
    assert_eq!(row["ip"], "127.0.0.1");
    assert!(row.get("address").is_none());
    assert_eq!(rows["peers"][0], *row);
    let file_id = "id +/?#&中文%";
    async fn file(server: &crate::server::ContentServer, id: &str) -> plain_graphql::Response {
        server
            .native_graphql()
            .unwrap()
            .execute(
                plain_graphql::Request::new(
                    r#"query($file:String!){peerFileUrl(peerId:"fixture",fileId:$file)}"#,
                )
                .variables(plain_graphql::Variables::from_json(json!({"file":id}))),
            )
            .await
    }
    let response = file(&server, file_id).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let parsed = http_client::Url::parse(data["peerFileUrl"].as_str().unwrap()).unwrap();
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert_eq!(parsed.path(), "/fs");
    assert_eq!(
        parsed.query_pairs().collect::<Vec<_>>(),
        vec![
            ("id".into(), file_id.into()),
            ("action".into(), "download".into())
        ]
    );
    peer.ip = "::1".into();
    peer.port = Some(2443);
    plain_rs::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    let response = file(&server, file_id).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let url = response.data.into_json().unwrap()["peerFileUrl"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(url.starts_with("https://[::1]:2443/fs?"), "{url}");
    assert_eq!(peer.base_url().unwrap(), "https://[::1]:2443");
    plain_rs::infrastructure::db::chat_store::peers::delete(&state.db, &[peer.id.clone()]).unwrap();
    assert_eq!(
        file(&server, file_id).await.errors[0].message,
        "Peer removed"
    );
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}
