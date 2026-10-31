use crate::server::ContentServer;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn existing_file_queries_authorize_effective_parent_and_own_pages_counts_and_zip_filtering() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("allowed");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("a.txt"), b"1234").unwrap();
    std::fs::write(root.join("b.txt"), b"12").unwrap();
    let denied = temp.path().join("denied");
    std::fs::create_dir(&denied).unwrap();
    std::fs::write(denied.join("private.txt"), b"private").unwrap();
    let server = ContentServer::open(&temp.path().join("core")).unwrap();
    let state = server.runtime_state().core;
    state.file_platform.use_sdk_for_test();
    let paths = Arc::new(Mutex::new(Vec::<String>::new()));
    let facts = paths.clone();
    let revoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let deny = revoked.clone();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let worker = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            assert_eq!(request["method"], "systemPermissionFacts");
            assert_eq!(
                request["params"]["permissions"],
                json!(["WRITE_EXTERNAL_STORAGE"])
            );
            facts.lock().unwrap().push("WRITE_EXTERNAL_STORAGE".into());
            let response = json!({"id":request["id"],"result":{"granted":{"WRITE_EXTERNAL_STORAGE":!deny.load(std::sync::atomic::Ordering::SeqCst)}}});
            host.reply(generation, response).unwrap();
        }
    });
    let native = server.native_graphql().unwrap();
    let query = "query($root:String!,$query:String!,$offset:Int!,$countOnly:Boolean!){count:fileCount(root:$root,query:$query) items:files(root:$root,query:$query,offset:$offset,limit:1,sortBy:NAME_ASC) @skip(if:$countOnly){name size path}}";
    async fn call(
        native: &crate::native::graphql::NativeGraphqlExecutor,
        query: &str,
        variables: Value,
    ) -> async_graphql::Response {
        native
            .execute(
                async_graphql::Request::new(query)
                    .variables(async_graphql::Variables::from_json(variables)),
            )
            .await
    }
    let mut variables = json!({"root":root,"query":"","offset":1,"countOnly":false});
    let response = call(&native, query, variables.clone()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let page = response.data.into_json().unwrap();
    assert_eq!(page["count"], 2);
    assert_eq!(page["items"][0]["name"], "b.txt");
    variables["countOnly"] = json!(true);
    let response = call(&native, query, variables.clone()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let page = response.data.into_json().unwrap();
    assert_eq!(page["count"], 2);
    assert!(page.get("items").is_none());
    revoked.store(true, std::sync::atomic::Ordering::SeqCst);
    variables["query"] = json!(format!("parent:{}", json!(denied)));
    let response = call(&native, query, variables.clone()).await;
    assert_eq!(response.errors.len(), 1);
    assert!(response.errors[0].message.contains("denied"));
    revoked.store(false, std::sync::atomic::Ordering::SeqCst);
    let archive = temp.path().join("archive.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    use std::io::Write;
    for (name, content) in [
        ("small", b"12".as_slice()),
        ("large", b"12345678".as_slice()),
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap();
    variables = json!({"root":format!("{}!zip!/",archive.display()),"query":"file_size:>4","offset":0,"countOnly":false});
    let response = call(&native, query, variables).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let page = response.data.into_json().unwrap();
    assert_eq!(page["count"], 1);
    assert_eq!(page["items"][0]["name"], "large");
    assert!(paths.lock().unwrap().len() >= 4);
    server.shutdown().await;
    worker.abort();
}
