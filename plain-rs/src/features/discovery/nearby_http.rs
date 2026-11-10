pub use crate::features::discovery::nearby_wire::Message;
use anyhow::Result;
use plain_libs::protocols::http_client;
use std::{sync::OnceLock, time::Duration};

fn client() -> Result<&'static http_client::Client> {
    static CLIENT: OnceLock<Result<http_client::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            http_client::builder()
                .no_proxy()
                .danger_accept_invalid_certs(true)
                .connect_timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.to_owned()))
}
/// Endpoint the nearby wire speaks to. Shared with the TLS listener
/// tests in plain-server, which assert against the same URL.
pub fn url(ip: &str, port: u16) -> Result<String> {
    anyhow::ensure!(port != 0, "Missing nearby port");
    let host = ip.trim().trim_start_matches('[').trim_end_matches(']');
    anyhow::ensure!(
        crate::infrastructure::transport::lan::valid_host(host),
        "Invalid nearby host"
    );
    Ok(plain_libs::utils::http_url::build_url(
        "https", host, port, "/nearby",
    ))
}
/// POST a raw wire body; `false` on transport failure, timeout or a
/// non-success status. `timeout` is the per-call deadline.
pub async fn post_url(
    url: &str,
    body: String,
    timeout: Duration,
    client_id: Option<&str>,
) -> Result<bool> {
    let mut request = client()?
        .post(url)
        .header(http_client::header::CONTENT_TYPE, "application/json")
        .timeout(timeout)
        .body(body);
    if let Some(client_id) = client_id {
        request = request.header("c-id", client_id);
    }
    let result = request.send().await;
    Ok(result.is_ok_and(|response| response.status().is_success()))
}
pub async fn send(ip: &str, port: u16, message: &Message, client_id: Option<&str>) -> Result<bool> {
    post_url(
        &url(ip, port)?,
        message.wire()?,
        Duration::from_secs(5),
        client_id,
    )
    .await
}
pub async fn probe(ip: &str, port: u16) -> Result<bool> {
    post_url(
        &url(ip, port)?,
        "DISCOVER:".into(),
        Duration::from_millis(2500),
        None,
    )
    .await
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/discovery/nearby_http.rs"]
mod tests;
