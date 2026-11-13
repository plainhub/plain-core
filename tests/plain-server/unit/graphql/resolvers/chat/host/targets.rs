use super::*;

#[tokio::test]
async fn peer_targets_require_prefix_before_effects() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let content = json!({"type":"TEXT","value":{"text":"fixture"}}).to_string();
    let source = execute(&server, format!(
        "mutation {{ sendChatItems(targets:[\"peer:local\"],content:{}) {{id}} }}", json!(content)
    )).await;
    let id = json!(source["sendChatItems"][0]["id"].as_str().unwrap());
    let folder = json!(dir.path().to_string_lossy());
    for raw in ["local", "fixture-peer", "unknown:id", "", "peer:", "channel:"] {
        let target = json!(raw);
        let targets = json!(["peer:local", raw]);
        let documents = [
            format!("{{ chatItems(target:{target},query:\"\",offset:0,limit:10) {{id}} }}"),
            format!("mutation {{ sendChatItems(targets:{targets},content:{}) {{id}} }}", json!(content)),
            format!("mutation {{ forwardChatItems(id:{id},targets:{targets}) {{id}} }}"),
            format!("mutation {{ sendShare(shareId:\"missing\",targets:{targets}) {{id}} }}"),
            format!("mutation {{ sendFolderShare(path:{folder},name:\"fixture\",targets:{targets}) {{id}} }}"),
        ];
        for document in documents {
            let response = server.graphql_executor().execute(Request::new(&document)).await;
            assert!(!response.errors.is_empty(), "accepted {raw:?}: {document}");
            if !raw.is_empty() && raw != "peer:" && raw != "channel:" {
                assert!(response.errors.iter().any(|error| error.message.contains("Target must use peer:<id> or channel:<id>")), "{:?}", response.errors);
            }
            assert_eq!(state.db.get_all_chats().len(), 1, "partial send: {document}");
            assert!(state.db.share_list().unwrap().is_empty(), "partial share: {document}");
        }
    }
    let rows = execute(&server, "{ chatItems(target:\"peer:local\",query:\"\",offset:0,limit:10) {id toId} }".into()).await;
    assert_eq!(rows["chatItems"].as_array().unwrap().len(), 1);
    assert_eq!(rows["chatItems"][0]["toId"], "local");
    server.shutdown().await;
}
