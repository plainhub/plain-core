use plain_libs::protocols::http_client;
use super::*;
use plain_libs::axum::{
    Router,
    body::Body,
    http::{HeaderMap, Response, StatusCode, header},
    response::Html,
    routing::get,
};
async fn fixture_server() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let server=Router::new().route("/", get(fixture_page))
        .route("/missing-image",get(||async{Html("<title>Missing image</title><meta property='og:image' content='/missing.png'>")}))
        .route("/favicon.png",get(||async{Response::builder().header(header::CONTENT_TYPE,"image/png").body(Body::from(plain_libs::utils::base64::base64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/lYUAAAAASUVORK5CYII="))).unwrap()}))
        .route("/chunked",get(||async{Response::builder().header(header::CONTENT_TYPE,"text/html").body(Body::from_stream(futures_util::stream::iter((0..4).map(|_|Ok::<_,std::io::Error>(vec![b'x';16]))))).unwrap()}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        plain_libs::axum::serve(listener, server).await.unwrap();
    });
    (addr, task)
}

async fn fixture_page(headers: HeaderMap) -> (StatusCode, Html<&'static str>) {
    if !headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("PlainApp-LinkPreview/"))
    {
        return (StatusCode::NOT_IMPLEMENTED, Html("Request rejected"));
    }
    (
        StatusCode::OK,
        Html("<meta property='og:title' content='Fixture title'><meta property='og:description' content='Fixture description'><meta property='og:image' content='/favicon.png'>"),
    )
}

#[tokio::test]
async fn html_image_fetch_and_atomic_message_commit_use_same_rust_pipeline() {
    let (addr, server) = fixture_server().await;
    let client = preview_client_builder()
        .no_proxy()
        .resolve("fixture.example", addr)
        .build()
        .unwrap();
    let url = format!("http://fixture.example:{}/", addr.port());
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let attachment = json!({"type":"FILE","attachment":{"id":"attachment","uri":"fsid:fixture","size":1}});
    let mut row = crate::infrastructure::db::DChat::new(
        "me",
        "local",
        "",
        &json!({"type":"MESSAGE","value":{"format":"PLAIN","items":[attachment.clone()],"text":url}}).to_string(),
    );
    row.id = "message".into();
    db.insert_chat(&row).unwrap();
    let row = refresh_with(&db, dir.path(), "message", &client)
        .await
        .unwrap()
        .expect("a successful page response should backfill the stored message");
    let content: Value = plain_libs::serde_json::from_str(&row.content).unwrap();
    assert_eq!(content["value"]["items"], json!([attachment]));
    let mut markdown = row.clone();
    markdown.id = "markdown".into();
    let mut markdown_content = content.clone();
    markdown_content["value"]["format"] = json!("MARKDOWN");
    markdown_content["value"].as_object_mut().unwrap().remove("linkPreviews");
    markdown.content = markdown_content.to_string();
    db.insert_chat(&markdown).unwrap();
    assert!(refresh_with(&db, dir.path(), &markdown.id, &client).await.unwrap().is_none());
    let preview = &content["value"]["linkPreviews"][0];
    assert_eq!(preview["title"], "Fixture title");
    assert_eq!(preview["description"], "Fixture description");
    assert!(plain_libs::serde_json::from_value::<crate::infrastructure::types::Instant>(
        preview["createdAt"].clone(),
    ).is_ok());
    assert!(preview.get("imageUrl").is_none());
    assert!(preview.get("imageLocalPath").is_none());
    assert_eq!(preview["imageWidth"], 1);
    let hash = preview["image"]
        .as_str()
        .unwrap()
        .strip_prefix("fid:")
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    let file = db.app_file_get(hash).unwrap().unwrap();
    assert_eq!(file.ref_count, 1);
    assert_eq!(
        std::fs::read(dir.path().join(&file.real_path)).unwrap(),
        plain_libs::utils::base64::base64_decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/lYUAAAAASUVORK5CYII="
        )
    );
    assert!(
        refresh_with(&db, dir.path(), "message", &client)
            .await
            .unwrap()
            .is_none()
    );
    let missing_url = format!("http://fixture.example:{}/missing-image", addr.port());
    let missing = crate::infrastructure::db::DChat::new("me", "local", "", &json!({
        "type":"MESSAGE","value":{"format":"PLAIN","items":[],"text":missing_url}
    }).to_string());
    db.insert_chat(&missing).unwrap();
    let missing = refresh_with(&db, dir.path(), &missing.id, &client).await.unwrap().unwrap();
    let missing: Value = plain_libs::serde_json::from_str(&missing.content).unwrap();
    let no_image = &missing["value"]["linkPreviews"][0];
    assert_eq!(no_image["title"], "Missing image");
    assert_eq!(no_image["image"], Value::Null);
    assert!(plain_libs::serde_json::from_value::<crate::infrastructure::types::Instant>(
        no_image["createdAt"].clone(),
    ).is_ok());
    assert!(no_image.get("imageUrl").is_none());
    assert!(no_image.get("imageLocalPath").is_none());
    edit(&db, dir.path(), "message", "plain text").unwrap();
    assert!(db.app_file_get(hash).unwrap().is_none());
    server.abort();
}
#[tokio::test]
async fn missing_content_length_cannot_bypass_body_limit() {
    let (addr, server) = fixture_server().await;
    let response = http_client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{addr}/chunked"))
        .send()
        .await
        .unwrap();
    assert!(read_bounded(response, 32).await.is_err());
    server.abort();
}
