use crate::server::ContentServer;
use plain_graphql::Request;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

#[tokio::test]
async fn main_share_content_uses_saved_roots_identity_and_live_port() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    let file = dir.path().join("shared.txt");
    std::fs::write(&file, b"abc").unwrap();
    let (_generation, mut requests) = server.runtime_state().host.connect();
    let executor = server.graphql_executor();
    let result = executor.execute(Request::new(format!(
        "mutation {{ createShare(input: {{ name: \"A share\", password: \"\", data: [{{ virtualPath: \"/shared.txt\", realPath: {}, isDir: false }}], readOnly: true }}) {{ id }} }}", json!(file.to_str().unwrap())
    ))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    let id = data["createShare"]["id"].as_str().unwrap();
    let result = executor
        .execute(Request::new(format!(
            r#"mutation {{ sendShare(shareId:{}, targets:["peer:local"]) {{content}} }}"#,
            json!(id)
        )))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    let message: Value =
        plain_libs::serde_json::from_str(data["sendShare"][0]["content"].as_str().unwrap())
            .unwrap();
    assert_eq!(message["type"], "SHARE");
    assert_eq!(message["value"]["shareId"], id);
    assert_eq!(
        message["value"]["urlToken"],
        plain_rs::features::files::shares::Service::new(
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        Arc::new(
            plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
        ),
    )
    .unwrap();
    let shared = directory.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::write(shared.join("file.txt"), b"abc").unwrap();
    let state = server.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let result = server.graphql_executor().execute(Request::new(r#"mutation($path:String!){sendFolderShare(targets:["peer:local"],path:$path,name:"Folder"){id content status}}"#)
        .variables(plain_graphql::Variables::from_json(json!({"path":shared})))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["sendFolderShare"][0]["status"], "SENT");
    let share = state.db.share_list().unwrap().pop().unwrap();
    let result = server.graphql_executor().execute(Request::new(r#"mutation($id:ID!){sendShare(targets:["peer:local","peer:local"],shareId:$id){id content status}}"#)
        .variables(plain_graphql::Variables::from_json(json!({"id":share.id})))).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.data.into_json().unwrap()["sendShare"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let result = server.graphql_executor().execute(Request::new(r#"mutation($content:String!){sendChatItems(targets:["peer:local","peer:local"],content:$content){id status}}"#)
        .variables(plain_graphql::Variables::from_json(json!({"content":json!({"type":"TEXT","value":{"text":"Hello"}}).to_string()})))).await;
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

#[tokio::test]
async fn share_crud_preserves_aliases_and_rejects_invalid_replacements_atomically() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let file = directory.path().join("original.txt");
    std::fs::write(&file, b"original").unwrap();
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let executor = server.graphql_executor();
    let projection = "id name password expiresAt readOnly data { virtualPath realPath isDir } url";
    let input = json!({"name":"Original","password":"secret","expiresAt":"2099-01-01T00:00:00Z","readOnly":true,"data":[{"virtualPath":"/alias.txt","realPath":file,"isDir":false}]});
    let created = executor
        .execute(
            Request::new(format!(
                "mutation($input:ShareInput!){{createShare(input:$input){{{projection}}}}}"
            ))
            .variables(plain_graphql::Variables::from_json(json!({"input":input}))),
        )
        .await;
    assert!(created.errors.is_empty(), "{:?}", created.errors);
    let created = created.data.into_json().unwrap()["createShare"].clone();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["data"][0]["virtualPath"], "/alias.txt");
    assert_eq!(created["password"], "secret");
    let old_url = created["url"].clone();
    for data in [
        json!([]),
        json!([{"virtualPath":"/alias.txt","realPath":file,"isDir":true}]),
        json!([{"virtualPath":"/../outside","realPath":file,"isDir":false}]),
        json!([{"virtualPath":"/alias.txt","realPath":file,"isDir":false},{"virtualPath":"alias.txt/","realPath":file,"isDir":false}]),
    ] {
        let mut invalid = input.clone();
        invalid["data"] = data;
        let result = executor
            .execute(
                Request::new(
                    "mutation($id:ID!,$input:ShareInput!){updateShare(id:$id,input:$input){id}}",
                )
                .variables(plain_graphql::Variables::from_json(
                    json!({"id":id,"input":invalid}),
                )),
            )
            .await;
        assert!(!result.errors.is_empty());
        let result = executor
            .execute(
                Request::new(format!("query($id:ID!){{share(id:$id){{{projection}}}}}"))
                    .variables(plain_graphql::Variables::from_json(json!({"id":id}))),
            )
            .await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.data.into_json().unwrap()["share"], created);
    }
    let mut replacement = input;
    replacement["name"] = json!("Updated");
    replacement["password"] = json!("");
    replacement["expiresAt"] = Value::Null;
    replacement["readOnly"] = json!(false);
    let updated = executor.execute(Request::new(format!("mutation($id:ID!,$input:ShareInput!){{updateShare(id:$id,input:$input){{{projection}}}}}"))
        .variables(plain_graphql::Variables::from_json(json!({"id":id,"input":replacement})))).await;
    assert!(updated.errors.is_empty(), "{:?}", updated.errors);
    let updated = updated.data.into_json().unwrap()["updateShare"].clone();
    assert_eq!(updated["name"], "Updated");
    assert_eq!(updated["password"], "");
    assert_eq!(updated["readOnly"], false);
    assert!(updated["expiresAt"].is_null());
    assert_eq!(updated["url"], old_url);
    assert_eq!(updated["data"], created["data"]);
    let result = executor.execute(Request::new("query{shares{id}} ")).await;
    assert!(result.errors.is_empty());
    assert_eq!(result.data.into_json().unwrap()["shares"][0]["id"], id);
    for expected in [true, false] {
        let result = executor
            .execute(
                Request::new("mutation($id:ID!){deleteShare(id:$id)}")
                    .variables(plain_graphql::Variables::from_json(json!({"id":id}))),
            )
            .await;
        assert!(result.errors.is_empty());
        assert_eq!(result.data.into_json().unwrap()["deleteShare"], expected);
    }
    let result = executor
        .execute(
            Request::new("query($id:ID!){share(id:$id){id}}")
                .variables(plain_graphql::Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(result.errors.is_empty());
    assert!(result.data.into_json().unwrap()["share"].is_null());
    assert_eq!(std::fs::read(file).unwrap(), b"original");
    server.shutdown().await;
}
