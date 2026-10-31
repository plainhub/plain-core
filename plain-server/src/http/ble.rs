use crate::server::ServerState;
use anyhow::{Result, ensure};
use axum::body::Body;
use plain_rs::protocols::ble;
use plain_rs::protocols::ble::Request;

mod nearby;

fn matches_characteristic(value: &str) -> bool {
    value.eq_ignore_ascii_case(ble::HTTP_CHAR_UUID)
        || value.eq_ignore_ascii_case(ble::NEARBY_CHAR_UUID)
}

pub(crate) async fn dispatch_incoming(
    state: ServerState,
    remote_host: String,
    characteristic_uuid: String,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    ensure!(
        !remote_host.is_empty() && matches_characteristic(&characteristic_uuid),
        "Invalid BLE connection facts"
    );
    ensure!(
        remote_host.len() + characteristic_uuid.len() <= 4096,
        "BLE facts exceed limit"
    );
    ble::parts(bytes)?;
    if characteristic_uuid.eq_ignore_ascii_case(ble::NEARBY_CHAR_UUID) {
        nearby::dispatch(&state, remote_host, bytes).await
    } else {
        dispatch(state, bytes).await
    }
}

pub(crate) async fn dispatch(state: ServerState, data: &[u8]) -> Result<Vec<u8>> {
    let request = Request::decode(data)?;
    let (mut builder, body) = match request {
        Request::PeerGraphql {
            client_id,
            channel_id,
            body,
        } => (
            axum::http::Request::builder()
                .method("POST")
                .uri("/peer_graphql")
                .header("c-id", client_id)
                .header("c-cid", channel_id),
            body,
        ),
        Request::FileChunk {
            client_id,
            file_id,
            offset,
            length,
        } => {
            let mut url = reqwest::Url::parse("http://localhost/fs")?;
            url.query_pairs_mut()
                .append_pair("id", &file_id)
                .append_pair("offset", &offset.to_string())
                .append_pair("length", &length.to_string());
            (
                axum::http::Request::builder()
                    .method("GET")
                    .uri(format!("/fs?{}", url.query().unwrap()))
                    .header("c-id", client_id)
                    .header("c-cid", ""),
                Vec::new(),
            )
        }
    };
    builder = builder.header("content-type", "application/octet-stream");
    let mut req = builder.body(Body::from(body))?;
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    #[cfg(feature = "http_transport")]
    {
        req.extensions_mut()
            .insert(crate::http::ConnectionScheme("https"));
    }
    use tower::ServiceExt;
    let response = crate::http::router::transfer_routes(state)
        .oneshot(req)
        .await?;
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), ble::MAX_BODY).await?;
    ble::response(status, &body)
}

#[cfg(test)]
#[path = "../../tests/unit/http/ble.rs"]
mod tests;
