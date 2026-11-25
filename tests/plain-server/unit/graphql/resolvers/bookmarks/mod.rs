use crate::server::ContentServer;
use crate::test_support::{graphql_http as call, graphql_http_variables as call_variables};
use plain_rs::infrastructure::db::Db;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

async fn metadata_updated(
    events: &mut tokio::sync::broadcast::Receiver<plain_rs::infrastructure::events::WsEvent>,
    id: &str,
) -> plain_libs::serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        for _ in 0..64 {
            let event = events.recv().await.unwrap();
            if event.event_type == plain_rs::infrastructure::events::WS_BOOKMARK_UPDATED {
                let rows: plain_libs::serde_json::Value =
                    plain_libs::serde_json::from_str(&event.payload).unwrap();
                if rows[0]["id"] == id {
                    return rows[0].clone();
                }
            }
        }
        panic!("Bookmark metadata update missing");
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn bookmark_http_lifecycle_metadata_and_reference_counts() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("plain.db");
    let token = plain_libs::utils::base64::base64_encode(&[10; 32]);
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs.clone()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let icon_requests = Arc::new(AtomicUsize::new(0));
    let pages = Arc::new(tokio::sync::Semaphore::new(0));
    let mut events = server.runtime_state().events.subscribe();
    let router = plain_libs::axum::Router::new()
        .route("/page", plain_libs::axum::routing::get({
            let pages = pages.clone();
            move || {
                let pages = pages.clone();
                async move {
                    pages.acquire().await.unwrap().forget();
                    plain_libs::axum::response::Html("<title>fallback</title><meta content='Hello &amp; 🌍' property='og:title'><link href='icon' rel='shortcut icon'>")
                }
            }
        }))
        .route("/icon", plain_libs::axum::routing::get({
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
        plain_libs::axum::serve(listener, router).await.unwrap();
    });
    let group = call(
        server.port,
        &token,
        "mutation {createBookmarkGroup(name:\"Links\") {id}}",
    )
    .await;
    let gid = group["data"]["createBookmarkGroup"]["id"].as_str().unwrap();
    let created = tokio::time::timeout(std::time::Duration::from_secs(2), call(server.port, &token, &format!("mutation {{addBookmarks(urls:[\"{origin}/page\",\" \"],groupId:\"{gid}\") {{id title faviconPath}}}}"))).await.expect("Creation must return while metadata is blocked");
    assert!(created.get("errors").is_none(), "{created}");
    assert_eq!(created["data"]["addBookmarks"].as_array().unwrap().len(), 1);
    let id = created["data"]["addBookmarks"][0]["id"].as_str().unwrap();
    let db = Arc::new(Db::open(&path).unwrap());
    let initial = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
        .unwrap()
        .unwrap();
    assert_eq!(initial.title, format!("{origin}/page"));
    assert_eq!(initial.favicon_path, "");
    assert_eq!(created["data"]["addBookmarks"][0]["title"], initial.title);
    assert_eq!(created["data"]["addBookmarks"][0]["faviconPath"], "");
    assert_eq!(icon_requests.load(Ordering::SeqCst), 0);
    pages.add_permits(64);
    let updated = metadata_updated(&mut events, id).await;
    assert_eq!(updated["title"], "Hello & 🌍");
    let before = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
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
    assert_eq!(before.title, "Hello & 🌍");
    assert!(before.favicon_path.starts_with("fid:"));
    let created_icon = before.favicon_path.clone();
    let file_id = created_icon
        .strip_prefix("fid:")
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    let icon_path = dir
        .path()
        .join(&db.get_app_file(file_id).unwrap().real_path);
    assert_eq!(std::fs::read(&icon_path).unwrap(), vec![1u8, 2, 3, 4]);
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    assert_eq!(updated["faviconPath"], created_icon);
    let files_before = count_files();
    let metadata_query = "query($url: String!) {pageMetadata(url:$url) {title faviconUrl}}";
    let metadata_variables = plain_libs::serde_json::json!({"url": format!("{origin}/page")});
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
    assert_eq!(icon_requests.load(Ordering::SeqCst), 1);
    let unchanged = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
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
        plain_libs::serde_json::json!({"id": id}),
    )
    .await;
    assert_eq!(read["data"]["bookmark"]["title"], before.title);
    assert_eq!(read["data"]["bookmark"]["faviconPath"], created_icon);
    let missing = call_variables(
        server.port,
        &token,
        "query($id: ID!) {bookmark(id:$id) {id}}",
        plain_libs::serde_json::json!({"id": "missing"}),
    )
    .await;
    assert_eq!(
        missing["data"]["bookmark"],
        plain_libs::serde_json::Value::Null
    );
    let invalid_page = call_variables(
        server.port,
        &token,
        metadata_query,
        plain_libs::serde_json::json!({"url": "file:///page.html"}),
    )
    .await;
    assert!(invalid_page.get("errors").is_some(), "{invalid_page}");

    let update_query = "mutation($id: ID!, $input: BookmarkInput!) {updateBookmark(id:$id,input:$input) {id title faviconPath clickCount}}";
    let input = plain_libs::serde_json::json!({
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
        plain_libs::serde_json::json!({"id": id, "input": input}),
    )
    .await;
    assert!(saved.get("errors").is_none(), "{saved}");
    assert_eq!(saved["data"]["updateBookmark"]["title"], "Chosen title");
    let icon = saved["data"]["updateBookmark"]["faviconPath"]
        .as_str()
        .unwrap();
    assert_eq!(icon, created_icon);
    assert!(icon_path.exists());
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    let duplicate = call_variables(
        server.port,
        &token,
        "mutation($url: String!) {addBookmarks(urls:[$url],groupId:null) {id}}",
        plain_libs::serde_json::json!({"url": format!("{origin}/page")}),
    )
    .await;
    assert_eq!(
        duplicate["data"]["addBookmarks"],
        plain_libs::serde_json::json!([])
    );
    let second_url = format!("{origin}/page?second");
    let second = call_variables(
        server.port,
        &token,
        "mutation($url: String!, $group: ID!) {addBookmarks(urls:[$url],groupId:$group) {id}}",
        plain_libs::serde_json::json!({"url": second_url, "group": gid}),
    )
    .await;
    let second_id = second["data"]["addBookmarks"][0]["id"].as_str().unwrap();
    metadata_updated(&mut events, second_id).await;
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let mut second_input = input.clone();
    second_input["url"] = plain_libs::serde_json::json!(second_url);
    let saved_second = call_variables(
        server.port,
        &token,
        update_query,
        plain_libs::serde_json::json!({"id": second_id, "input": second_input}),
    )
    .await;
    assert!(saved_second.get("errors").is_none(), "{saved_second}");
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let mut failed_input = input.clone();
    failed_input["title"] = plain_libs::serde_json::json!("Must not persist");
    failed_input["faviconUrl"] = plain_libs::serde_json::json!(format!("{origin}/missing-icon"));
    let failed = call_variables(
        server.port,
        &token,
        update_query,
        plain_libs::serde_json::json!({"id": id, "input": failed_input}),
    )
    .await;
    assert!(failed.get("errors").is_some(), "{failed}");
    assert_eq!(
        plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .title,
        "Chosen title"
    );
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let mut clear_input = second_input.clone();
    clear_input["faviconUrl"] = plain_libs::serde_json::json!("");
    let cleared = call_variables(
        server.port,
        &token,
        update_query,
        plain_libs::serde_json::json!({"id": second_id, "input": clear_input}),
    )
    .await;
    assert_eq!(cleared["data"]["updateBookmark"]["faviconPath"], "");
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    let restored = call_variables(
        server.port,
        &token,
        update_query,
        plain_libs::serde_json::json!({"id": second_id, "input": second_input}),
    )
    .await;
    assert!(restored.get("errors").is_none(), "{restored}");
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);
    let repeated = call_variables(server.port, &token, metadata_query, metadata_variables).await;
    assert_eq!(repeated["data"], meta["data"]);
    assert_eq!(
        plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .title,
        "Chosen title"
    );
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 2);

    let query = format!("mutation {{recordBookmarkClick(id:\"{id}\")}}");
    futures_util::future::join_all((0..20).map(|_| call(server.port, &token, &query))).await;
    assert_eq!(
        plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
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
    let edited = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
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
        plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id)
            .unwrap()
            .unwrap()
            .group_id,
        None
    );
    let stale = call(server.port, &token, &edit.replace("Edited", "Stale edit")).await;
    assert!(stale.get("errors").is_some(), "{stale}");
    let retained = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&db, id).unwrap().unwrap();
    assert_eq!(retained.title, "Edited");
    assert!(retained.group_id.is_none());
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
    assert!(icon_path.exists());
    assert_eq!(db.get_app_file(file_id).unwrap().ref_count, 1);
    call(
        server.port,
        &token,
        &format!("mutation {{deleteBookmarks(ids:[\"{second_id}\"]) {{affectedCount}}}}"),
    )
    .await;
    assert!(!icon_path.exists());
    assert!(db.get_app_file(file_id).is_none());
    server.shutdown().await;
    web.abort();
}

#[tokio::test]
async fn delete_bookmark_group_emits_stored_rows_after_database_changes() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[19; 32]);
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    let server = ContentServer::start(directory.path(), &token, prefs).unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    use plain_rs::infrastructure::db::bookmark as store;
    let group = store::DBookmarkGroup::new("Owned group");
    store::insert_bookmark_group(&db, &group).unwrap();
    let mut selected = store::DBookmark::new("https://fixture.invalid/selected", Some(&group.id));
    selected.id = "owned".into();
    let neighbor = store::DBookmark::new("https://fixture.invalid/neighbor", None);
    store::insert_bookmark(&db, &selected).unwrap();
    store::insert_bookmark(&db, &neighbor).unwrap();
    db.with_conn(|conn| conn.execute_batch("CREATE TRIGGER observed_bookmark_change BEFORE UPDATE OF group_id ON bookmarks WHEN OLD.id='owned' BEGIN UPDATE bookmarks SET sort_order=42 WHERE id=OLD.id; END;")).unwrap();
    let mut events = server.runtime_state().events.subscribe();
    let response = call(
        server.port,
        &token,
        &format!("mutation {{deleteBookmarkGroup(id:\"{}\")}}", group.id),
    )
    .await;
    assert!(response.get("errors").is_none(), "{response}");
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        for _ in 0..32 {
            let event = events.recv().await.unwrap();
            if event.event_type == plain_rs::infrastructure::events::WS_BOOKMARK_UPDATED {
                return event;
            }
        }
        panic!("Bookmark event missing");
    })
    .await
    .unwrap();
    let emitted: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(&event.payload).unwrap();
    let stored = store::get_bookmark_by_id(&db, &selected.id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.group_id, None);
    assert_eq!(stored.sort_order, 42);
    assert_eq!(emitted.as_array().unwrap().len(), 1);
    assert_eq!(emitted[0].get("groupId"), Some(&plain_libs::serde_json::Value::Null));
    assert_eq!(emitted[0]["id"], selected.id);
    assert_eq!(emitted[0]["sortOrder"], stored.sort_order);
    assert_eq!(emitted[0]["updatedAt"], stored.updated_at);
    assert_eq!(
        plain_libs::serde_json::to_value(
            store::get_bookmark_by_id(&db, &neighbor.id)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        plain_libs::serde_json::to_value(neighbor).unwrap()
    );
    server.shutdown().await;
}

#[tokio::test]
async fn background_metadata_keeps_created_rows_and_successful_title_on_fetch_failure() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[21; 32]);
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    let server = ContentServer::start(directory.path(), &token, prefs).unwrap();
    let state = server.runtime_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = plain_libs::axum::Router::new().route(
        "/no-icon",
        plain_libs::axum::routing::get(|| async {
            plain_libs::axum::response::Html(
                "<title>Keep fetched title</title><link rel='icon' href='/missing.ico'>",
            )
        }),
    );
    let router = router
        .route(
            "/with-icon",
            plain_libs::axum::routing::get(|| async {
                plain_libs::axum::response::Html(
                    "<title>Fetched title</title><link rel='icon' href='/icon'>",
                )
            }),
        )
        .route(
            "/icon",
            plain_libs::axum::routing::get(|| async {
                (
                    [("content-type", "image/x-icon")],
                    b"independent icon fixture".to_vec(),
                )
            }),
        );
    let web = tokio::spawn(async move {
        plain_libs::axum::serve(listener, router).await.unwrap();
    });
    let rows = plain_rs::features::services::bookmarks::add_bookmarks(
        &state.db,
        vec![format!("{origin}/missing"), format!("{origin}/no-icon")],
        None,
    )
    .unwrap();
    plain_rs::features::services::bookmarks::start_metadata_update(
        state.core.clone(),
        rows.clone(),
    )
    .await
    .unwrap();
    let missing =
        plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&state.db, &rows[0].id)
            .unwrap()
            .unwrap();
    assert_eq!(missing.title, rows[0].url);
    assert_eq!(missing.favicon_path, "");
    let title = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&state.db, &rows[1].id)
        .unwrap()
        .unwrap();
    assert_eq!(title.title, "Keep fetched title");
    assert_eq!(title.favicon_path, "");
    assert_eq!(
        plain_rs::infrastructure::db::bookmark::get_bookmarks(&state.db).len(),
        2
    );
    assert_eq!(
        state
            .db
            .with_conn(
                |conn| conn.query_row("SELECT COUNT(*) FROM app_files", [], |row| row
                    .get::<_, i64>(0))
            )
            .unwrap(),
        0
    );
    let failed_save = plain_rs::features::services::bookmarks::add_bookmarks(
        &state.db,
        vec![format!("{origin}/with-icon")],
        None,
    )
    .unwrap();
    state.db.with_conn(|conn| conn.execute_batch("CREATE TRIGGER reject_metadata BEFORE UPDATE OF title ON bookmarks BEGIN SELECT RAISE(ABORT,'expected metadata storage failure'); END;")).unwrap();
    plain_rs::features::services::bookmarks::start_metadata_update(
        state.core.clone(),
        failed_save.clone(),
    )
    .await
    .unwrap();
    let unchanged =
        plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(&state.db, &failed_save[0].id)
            .unwrap()
            .unwrap();
    assert_eq!(unchanged.title, failed_save[0].url);
    assert_eq!(unchanged.favicon_path, "");
    assert_eq!(
        state
            .db
            .with_conn(
                |conn| conn.query_row("SELECT COUNT(*) FROM app_files", [], |row| row
                    .get::<_, i64>(0))
            )
            .unwrap(),
        0
    );
    server.shutdown().await;
    web.abort();
}

#[tokio::test]
async fn add_bookmarks_validates_group_and_accepts_null_or_omitted() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[19; 32]);
    let prefs = Arc::new(plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(directory.path(), &token, prefs).unwrap();
    let query = "mutation($urls:[String!]!,$groupId:ID) { addBookmarks(urls:$urls,groupId:$groupId){id groupId} }";
    let url = "http://127.0.0.1:1/page";
    let result = call_variables(server.port, &token, query, plain_libs::serde_json::json!({"urls":[url],"groupId":"missing"})).await;
    let db = &server.runtime_state().db;
    assert_eq!(db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM bookmarks", [], |r| r.get::<_, i32>(0)).unwrap()), 0);
    assert!(result.get("errors").is_some(), "{result}");
    for variables in [
        plain_libs::serde_json::json!({"urls":[url]}),
        plain_libs::serde_json::json!({"urls":[format!("{url}/other")],"groupId":null}),
    ] {
        let result = call_variables(server.port, &token, query, variables).await;
        assert!(result.get("errors").is_none(), "{result}");
        let row = &result["data"]["addBookmarks"][0];
        assert_eq!(row.get("groupId"), Some(&plain_libs::serde_json::Value::Null));
        let stored = plain_rs::infrastructure::db::bookmark::get_bookmark_by_id(db, row["id"].as_str().unwrap()).unwrap().unwrap();
        assert!(stored.group_id.is_none());
    }
    server.shutdown().await;
}
