use super::*;

#[test]
fn upload_info_defaults_match_the_desktop_payload() {
    let info: UploadInfo = serde_json::from_str("{}").unwrap();
    assert!(info.dir.is_empty());
    assert!(!info.replace);
    assert!(!info.is_app_file);
    assert_eq!(info.size, 0);
    let chunk: ChunkInfo = serde_json::from_str("{}").unwrap();
    assert!(chunk.file_id.is_empty());
    assert_eq!(chunk.index, 0);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn multipart_upload(
    core: &crate::assembly::server::ContentServer,
    endpoint: &str,
    client_id: &str,
    token: Option<&str>,
    key: &[u8],
    info: serde_json::Value,
    bytes: &[u8],
    file_first: bool,
) -> reqwest::Response {
    let boundary = "plain-upload-regression-boundary";
    let metadata = format!(
        "{}|{}|{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4(),
        info
    );
    let encrypted = plain_rs::crypto::xchacha_encrypt_raw(key, metadata.as_bytes()).unwrap();
    let mut metadata =
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"info\"\r\n\r\n")
            .into_bytes();
    metadata.extend_from_slice(&encrypted);
    metadata.extend_from_slice(b"\r\n");
    let mut file = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"part.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
    file.extend_from_slice(bytes);
    file.extend_from_slice(b"\r\n");
    let mut body = if file_first {
        let mut body = file;
        body.extend(metadata);
        body
    } else {
        metadata.extend(file);
        metadata
    };
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let chunks = body
        .chunks(32768)
        .map(|chunk| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(chunk)))
        .collect::<Vec<_>>();
    let mut request = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}{endpoint}", core.port))
        .header("c-id", client_id)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(
            chunks,
        )));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    request.send().await.unwrap()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn multipart_regular_upload_streams_without_host_preserves_replacements_and_validates_parts()
{
    let directory = tempfile::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("desktop_access", true).unwrap();
    let core = crate::assembly::server::ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let session = crate::services::sessions::create(&state, "upload fixture".into()).unwrap();
    let key = plain_rs::utils::base64::base64_decode(&session.token);
    let destination = directory.path().join("files");
    std::fs::create_dir(&destination).unwrap();
    let data = (0..(2 * 1024 * 1024 + 17))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let info = serde_json::json!({"dir":destination,"replace":false,"size":data.len()});
    let response = multipart_upload(
        &core,
        "/upload",
        &session.client_id,
        None,
        &key,
        info.clone(),
        &data,
        false,
    )
    .await;
    assert_eq!(response.status(), 201);
    assert_eq!(response.text().await.unwrap(), "part.bin");
    assert_eq!(std::fs::read(destination.join("part.bin")).unwrap(), data);
    let response = multipart_upload(
        &core,
        "/upload",
        &session.client_id,
        None,
        &key,
        info.clone(),
        &data,
        false,
    )
    .await;
    assert_eq!(response.status(), 201);
    assert_eq!(response.text().await.unwrap(), "part (1).bin");
    let invalid = serde_json::json!({"dir":destination,"replace":true,"size":10});
    let response = multipart_upload(
        &core,
        "/upload",
        &session.client_id,
        None,
        &key,
        invalid,
        b"short",
        false,
    )
    .await;
    assert_eq!(response.status(), 400);
    assert_eq!(std::fs::read(destination.join("part.bin")).unwrap(), data);
    assert!(std::fs::read_dir(&destination).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".upload_tmp")
    }));
    let response = multipart_upload(
        &core,
        "/upload",
        &session.client_id,
        None,
        &key,
        info.clone(),
        b"invalid",
        true,
    )
    .await;
    assert_eq!(response.status(), 400);
    let response =
        multipart_upload(&core, "/upload", "", None, &key, info, b"invalid", false).await;
    assert_eq!(response.status(), 401);
    assert!(requests.try_recv().is_err());
    core.shutdown().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_upload_chunks_use_the_main_identity_key_and_the_owned_chunk_directory() {
    let directory = tempfile::tempdir().unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    let prefs = std::sync::Arc::new(
        plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("desktop_access", true).unwrap();
    plain_rs::prefs::identity::ensure_url_token(&prefs);
    let core =
        crate::assembly::server::ContentServer::start(directory.path(), &token, prefs).unwrap();
    let state = core.runtime_state();
    let client_id = state.native_client_id().unwrap();
    let key = plain_rs::utils::base64::base64_decode(&core.native_url_token().unwrap());
    let data = vec![42; MAX_UPLOAD_WRITE_BYTES * 3 + 19];
    let info = serde_json::json!({"fileId":"fixture","index":0,"size":data.len()});
    let response = multipart_upload(
        &core,
        "/upload_chunk",
        &client_id,
        Some(&token),
        &key,
        info,
        &data,
        false,
    )
    .await;
    assert_eq!(response.status(), 201);
    let path = plain_rs::files::uploads::chunk_path(state.upload_chunks.directory(), "fixture", 0)
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), data);
    let info = serde_json::json!({"fileId":"fixture","index":0,"size":20});
    let response = multipart_upload(
        &core,
        "/upload_chunk",
        &client_id,
        Some(&token),
        &key,
        info,
        b"short",
        false,
    )
    .await;
    assert_eq!(response.status(), 400);
    assert_eq!(
        state.upload_chunks.list("fixture").await.unwrap(),
        vec![format!("0:{}", data.len())]
    );
    core.shutdown().await;
}
