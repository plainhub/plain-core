use crate::server::ContentServer;
use crate::test_support::{graphql_http as call, graphql_http_variables as call_variables};
use plain_rs::db::Db;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn bookmark_http_lifecycle_metadata_and_reference_counts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.db");
    let token = plain_rs::utils::base64::base64_encode(&[10; 32]);
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs.clone()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let icon_requests = Arc::new(AtomicUsize::new(0));
    let router = axum::Router::new()
        .route("/page", axum::routing::get(|| async { axum::response::Html("<title>fallback</title><meta content='Hello &amp; 🌍' property='og:title'><link href='icon' rel='shortcut icon'>") }))
        .route("/icon", axum::routing::get({
            let requests = icon_requests.clone();
            move || {
                let requests = requests.clone();
                async move {
                    requests.fetch_add(1, Ordering::SeqCst);
                    ([("content-type", "image/x-icon")], vec![1u8,2,3,4])
                }
            }
        }));
    let web = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let group = call(
        server.port,
        &token,
        "mutation {createBookmarkGroup(name:\"Links\") {id}}",
    )
    .await;
    let gid = group["data"]["createBookmarkGroup"]["id"].as_str().unwrap();
    let created = call(server.port, &token, &format!("mutation {{addBookmarks(urls:[\"{origin}/page\",\" \"],groupId:\"{gid}\") {{id title}}}}")).await;
    assert!(created.get("errors").is_none(), "{created}");
    assert_eq!(created["data"]["addBookmarks"].as_array().unwrap().len(), 1);
    let id = created["data"]["addBookmarks"][0]["id"].as_str().unwrap();
    let db = Arc::new(Db::open(&path).unwrap());
    let before = plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
        .unwrap()
        .unwrap();
    let count_files = || {
        db.with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM app_files", [], |row| {
                row.get::<_, i64>(0)
            })
        })
        .unwrap()
    };
    let files_before = count_files();
    let metadata_query = "query($url: String!) {pageMetadata(url:$url) {title faviconUrl}}";
    let metadata_variables = serde_json::json!({"url": format!("{origin}/page")});
    let meta = call_variables(
        server.port,
        &token,
        metadata_query,
        metadata_variables.clone(),
    )
    .await;
    assert!(meta.get("errors").is_none(), "{meta}");
    assert_eq!(meta["data"]["pageMetadata"]["title"], "Hello & 🌍");
    assert_eq!(
        meta["data"]["pageMetadata"]["faviconUrl"],
        format!("{origin}/icon")
    );
    let repeated = call_variables(
        server.port,
        &token,
        metadata_query,
        metadata_variables.clone(),
    )
    .await;
    assert_eq!(repeated["data"], meta["data"]);
    assert_eq!(icon_requests.load(Ordering::SeqCst), 0);
    let unchanged = plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.title, before.title);
    assert_eq!(unchanged.favicon_path, before.favicon_path);
    assert_eq!(unchanged.updated_at, before.updated_at);
    assert_eq!(count_files(), files_before);
    let read = call_variables(
        server.port,
        &token,
        "query($id: ID!) {bookmark(id:$id) {title faviconPath}}",
        serde_json::json!({"id": id}),
    )
    .await;
    assert_eq!(read["data"]["bookmark"]["title"], before.title);
    assert_eq!(read["data"]["bookmark"]["faviconPath"], "");
    let missing = call_variables(
        server.port,
        &token,
        "query($id: ID!) {bookmark(id:$id) {id}}",
        serde_json::json!({"id": "missing"}),
    )
    .await;
    assert_eq!(missing["data"]["bookmark"], serde_json::Value::Null);
    let invalid_page = call_variables(
        server.port,
        &token,
        metadata_query,
        serde_json::json!({"url": "file:///page.html"}),
    )
    .await;
    assert!(invalid_page.get("errors").is_some(), "{invalid_page}");

    let update_query = "mutation($id: ID!, $input: BookmarkInput!) {updateBookmark(id:$id,input:$input) {id title faviconPath clickCount}}";
    let input = serde_json::json!({
        "url": format!("{origin}/page"),
        "title": "Chosen title",
        "faviconUrl": meta["data"]["pageMetadata"]["faviconUrl"],
        "groupId": gid,
        "pinned": false,
        "sortOrder": 0,
    });
    let saved = call_variables(
        server.port,
        &token,
        update_query,
        serde_json::json!({"id": id, "input": input}),
    )
    .await;
    assert!(saved.get("errors").is_none(), "{saved}");
    assert_eq!(saved["data"]["updateBookmark"]["title"], "Chosen title");
    let icon = saved["data"]["updateBookmark"]["faviconPath"]
        .as_str()
        .unwrap();
    assert!(std::path::Path::new(icon).exists());
    let file_id = std::path::Path::new(icon)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    let second = call_variables(
        server.port,
        &token,
        "mutation($url: String!, $group: ID!) {addBookmarks(urls:[$url],groupId:$group) {id}}",
        serde_json::json!({"url": format!("{origin}/page"), "group": gid}),
    )
    .await;
    let second_id = second["data"]["addBookmarks"][0]["id"].as_str().unwrap();
    let saved_second = call_variables(
        server.port,
        &token,
        update_query,
        serde_json::json!({"id": second_id, "input": input}),
    )
    .await;
    assert!(saved_second.get("errors").is_none(), "{saved_second}");
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let mut failed_input = input.clone();
    failed_input["title"] = serde_json::json!("Must not persist");
    failed_input["faviconUrl"] = serde_json::json!(format!("{origin}/missing-icon"));
    let failed = call_variables(
        server.port,
        &token,
        update_query,
        serde_json::json!({"id": id, "input": failed_input}),
    )
    .await;
    assert!(failed.get("errors").is_some(), "{failed}");
    assert_eq!(
        plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .title,
        "Chosen title"
    );
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let mut clear_input = input.clone();
    clear_input["faviconUrl"] = serde_json::json!("");
    let cleared = call_variables(
        server.port,
        &token,
        update_query,
        serde_json::json!({"id": second_id, "input": clear_input}),
    )
    .await;
    assert_eq!(cleared["data"]["updateBookmark"]["faviconPath"], "");
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    let restored = call_variables(
        server.port,
        &token,
        update_query,
        serde_json::json!({"id": second_id, "input": input}),
    )
    .await;
    assert!(restored.get("errors").is_none(), "{restored}");
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let repeated = call_variables(server.port, &token, metadata_query, metadata_variables).await;
    assert_eq!(repeated["data"], meta["data"]);
    assert_eq!(
        plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .title,
        "Chosen title"
    );
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);

    let query = format!("mutation {{recordBookmarkClick(id:\"{id}\")}}");
    futures_util::future::join_all((0..20).map(|_| call(server.port, &token, &query))).await;
    assert_eq!(
        plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .click_count,
        20
    );
    let edit = format!(
        "mutation {{updateBookmark(id:\"{id}\",input:{{url:\"{origin}/page\",title:\"Edited\",groupId:\"{gid}\",pinned:true,sortOrder:2}}) {{title clickCount}}}}"
    );
    assert_eq!(
        call(server.port, &token, &edit).await["data"]["updateBookmark"]["clickCount"],
        20
    );
    let edited = plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
        .unwrap()
        .unwrap();
    assert_eq!(edited.title, "Edited");
    assert_eq!(edited.favicon_path, icon);
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let groups = call(server.port, &token, "{bookmarkGroups {itemCount}}").await;
    assert_eq!(groups["data"]["bookmarkGroups"][0]["itemCount"], 2);
    call(
        server.port,
        &token,
        &format!("mutation {{deleteBookmarkGroup(id:\"{gid}\")}}"),
    )
    .await;
    assert_eq!(
        plain_rs::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .group_id,
        None
    );
    assert_eq!(
        call(
            server.port,
            &token,
            "mutation {deleteBookmarks(ids:[]) {affectedCount}}"
        )
        .await["data"]["deleteBookmarks"]["affectedCount"],
        0
    );
    server.shutdown().await;
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs).unwrap();
    assert_eq!(
        call(server.port, &token, "{bookmarks {clickCount}}").await["data"]["bookmarks"][0]["clickCount"],
        20
    );
    assert_eq!(
        call(
            server.port,
            &token,
            &format!("mutation {{deleteBookmarks(ids:[\"{id}\"]) {{affectedCount}}}}")
        )
        .await["data"]["deleteBookmarks"]["affectedCount"],
        1
    );
    assert!(std::path::Path::new(icon).exists());
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    call(
        server.port,
        &token,
        &format!("mutation {{deleteBookmarks(ids:[\"{second_id}\"]) {{affectedCount}}}}"),
    )
    .await;
    assert!(!std::path::Path::new(icon).exists());
    assert!(db.get_app_file(file_id).is_none());
    server.shutdown().await;
    web.abort();
}
