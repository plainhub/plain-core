use plain_rs::db::DPeer;
use plain_rs::services::peers::peer_address::*;
use serde_json::json;
#[tokio::test]
async fn main_peer_records_and_file_urls_use_current_addresses_and_reject_stale_identity_without_host()
 {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let mut peer = DPeer::new(
        "fixture",
        " ",
        "bad, 127.0.0.1",
        Some(443),
        plain_rs::chat::enums::DeviceType::Tablet,
    );
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let native = server.native_graphql().unwrap();
    let response = native.execute(r#"{fixture:peers{ip address{bestIp name baseUrl apiUrl statusWsUrl}} peers{ip address{bestIp name baseUrl apiUrl statusWsUrl}}}"#.into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let rows = response.data.into_json().unwrap();
    let row = &rows["fixture"][0];
    assert_eq!(row["ip"], "bad, 127.0.0.1");
    assert_eq!(
        row["address"],
        json!({"bestIp":"127.0.0.1","name":"127.0.0.1","baseUrl":"https://127.0.0.1","apiUrl":"https://127.0.0.1/peer_graphql","statusWsUrl":"wss://127.0.0.1/status"})
    );
    assert_eq!(rows["peers"][0], *row);
    let file_id = "id +/?#&中文%";
    async fn file(
        server: &crate::server::ContentServer,
        expected: &DPeer,
        id: &str,
    ) -> async_graphql::Response {
        server.native_graphql().unwrap().execute(async_graphql::Request::new(r#"query($expected:PeerRecordInput!,$file:String!){peerFileUrl(id:"fixture",expected:$expected,fileId:$file)}"#).variables(async_graphql::Variables::from_json(json!({"expected":{"id":expected.id,"name":expected.name,"ip":expected.ip,"key":expected.key,"publicKey":expected.public_key,"status":expected.status,"port":expected.port,"deviceType":expected.device_type,"token":expected.token,"createdAt":expected.created_at,"updatedAt":expected.updated_at},"file":id})))).await
    }
    let response = file(&server, &peer, file_id).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let parsed = reqwest::Url::parse(data["peerFileUrl"].as_str().unwrap()).unwrap();
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert_eq!(parsed.path(), "/fs");
    assert_eq!(
        parsed.query_pairs().collect::<Vec<_>>(),
        vec![("id".into(), file_id.into())]
    );
    let old = peer.clone();
    peer.ip = "::1".into();
    peer.port = Some(2443);
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        plain_rs::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    assert_eq!(
        file(&server, &old, file_id).await.errors[0].message,
        "Peer changed"
    );
    let current = address(&peer);
    assert_eq!(current.api_url, "https://[::1]:2443/peer_graphql");
    assert_eq!(current.status_ws_url, "wss://[::1]:2443/status");
    plain_rs::db::chat_store::peers::delete(&state.db, &[peer.id.clone()]).unwrap();
    assert_eq!(
        file(&server, &peer, file_id).await.errors[0].message,
        "Peer removed"
    );
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}
