use super::{caller, caller_id, current, decode_key};
use crate::server::ServerState;
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::axum::{
    body::Bytes,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_libs::protocols::multipart::{Field, Multipart};
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
    let Some(decrypted) = plain_rs::infrastructure::transport::http_request::decrypt_raw(
        &key,
        "POST",
        &format!("/{path}"),
        &info,
    ) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(text) = std::str::from_utf8(&decrypted) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(text) = state.main_replay.admit(
        &state.db,
        &session.client_id,
        text,
        plain_libs::chrono::Utc::now().timestamp_millis(),
    ) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(route) = state.managed_resources.route(&state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if current(&state, &session).is_err() || route.check_current(&state).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(Some(field)) = multipart.next_field().await else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if field.name() != Some("file") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let (tx, rx) = mpsc::channel(2);
    let outgoing = crate::managed_peers::upload::request(
        state.clone(),
        route,
        path,
        text,
        field.file_name().unwrap_or_default(),
        field.content_type().unwrap_or("application/octet-stream"),
        rx,
    );
    let Ok(outgoing) = outgoing else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let mut outgoing = outgoing;
    let response = tokio::select! {
        biased;
        response = &mut outgoing => response,
        result = transfer(field, &tx) => {
            if result.is_err() {
                return StatusCode::BAD_REQUEST.into_response();
            }
            tokio::select! {
                biased;
                response = &mut outgoing => response,
                next = multipart.next_field() => {
                    if !matches!(next, Ok(None)) {
                        return StatusCode::BAD_REQUEST.into_response();
                    }
                    drop(tx);
                    outgoing.await
                }
            }
        }
    };
    let Ok((status, body)) = response else {
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

async fn transfer(mut field: Field<'_>, tx: &mpsc::Sender<io::Result<Bytes>>) -> Result<()> {
    while let Some(chunk) = field.chunk().await? {
        for offset in (0..chunk.len())
            .step_by(plain_rs::features::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES)
        {
            let end = (offset
                + plain_rs::features::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES)
                .min(chunk.len());
            tx.send(Ok(chunk.slice(offset..end))).await?;
        }
    }
    Ok(())
}
