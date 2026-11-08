use super::{GRAPHQL_LIMIT, MANAGED_RESOURCE, caller, caller_id, current, decode_key};
use crate::server::ServerState;
use anyhow::{Result, anyhow, ensure};
use axum::{
    body::Bytes,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_libs::protocols::{http_client, multipart::Multipart};
use plain_rs::infrastructure::transport::http_request;
use std::io;
use tokio::sync::mpsc;

pub(super) async fn call(
    state: ServerState,
    target: &str,
    path: &str,
    request: Request,
) -> Response {
    let Ok(session) = caller(&state, caller_id(request.headers())) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(key) = decode_key(&session.token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(mut multipart) = Multipart::from_request(request) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(info) = metadata(&mut multipart).await else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(decrypted) = plain_libs::crypto::xchacha_decrypt_raw(&key, &info) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(text) = std::str::from_utf8(&decrypted) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(text) = state.main_replay.admit(
        &session.client_id,
        text,
        chrono::Utc::now().timestamp_millis(),
    ) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(route) = state.managed_proxy.route(&state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if !route.supports_managed_resources {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    if current(&state, &session).is_err() || route.check_current(&state).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(encrypted) = http_request::encrypt(&route.key, text) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let boundary = plain_libs::utils::uuid::Uuid::new_v4().to_string();
    let Ok(url) = route.base.join(&format!("/{path}")) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let content_type = format!("multipart/form-data; boundary={boundary}");
    let (tx, rx) = mpsc::channel(2);
    let stream = futures_util::stream::unfold(rx, |mut rx| async {
        rx.recv().await.map(|chunk| (chunk, rx))
    });
    let request = route
        .client
        .post(url)
        .header("c-id", &route.client_id)
        .header(MANAGED_RESOURCE, "1")
        .header(header::CONTENT_TYPE, content_type)
        .body(http_client::Body::wrap_stream(stream));
    let mut outgoing = std::pin::pin!(request.send());
    let response = tokio::select! {
        biased;
        response = &mut outgoing => response,
        result = transfer(multipart, &boundary, encrypted, tx) => {
            if result.is_err() {
                return StatusCode::BAD_REQUEST.into_response();
            }
            outgoing.await
        }
    };
    let Ok(response) = response else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status = response.status();
    if status.is_redirection() {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let Ok(body) =
        plain_rs::features::services::peers::peer_lan::limited(response, GRAPHQL_LIMIT).await
    else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    (
        status,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

async fn metadata(multipart: &mut Multipart) -> Result<Vec<u8>> {
    let mut field = multipart
        .next_field()
        .await?
        .ok_or_else(|| anyhow!("Missing upload metadata"))?;
    ensure!(field.name() == Some("info"), "Expected upload metadata");
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 64 * 1024,
            "Upload metadata too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn transfer(
    mut multipart: Multipart,
    boundary: &str,
    info: Vec<u8>,
    tx: mpsc::Sender<io::Result<Bytes>>,
) -> Result<()> {
    let mut field = multipart
        .next_field()
        .await?
        .ok_or_else(|| anyhow!("Missing upload file"))?;
    ensure!(field.name() == Some("file"), "Expected upload file");
    let filename = field
        .file_name()
        .unwrap_or_default()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    ensure!(
        !filename.is_empty() && !filename.contains(['\r', '\n', '\0']),
        "Invalid upload filename"
    );
    let filename = filename.replace('"', "\\\"");
    let mime = field.content_type().unwrap_or("application/octet-stream");
    let prefix = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"info\"\r\n\r\n");
    let file = format!(
        "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n"
    );
    for bytes in [Bytes::from(prefix), Bytes::from(info), Bytes::from(file)] {
        tx.send(Ok(bytes)).await?;
    }
    while let Some(chunk) = field.chunk().await? {
        tx.send(Ok(chunk)).await?;
    }
    drop(field);
    ensure!(
        multipart.next_field().await?.is_none(),
        "Unexpected upload part"
    );
    tx.send(Ok(Bytes::from(format!("\r\n--{boundary}--\r\n"))))
        .await?;
    Ok(())
}
