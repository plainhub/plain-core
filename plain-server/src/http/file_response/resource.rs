use super::stream::Source;
use plain_libs::axum::{
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
pub(crate) async fn respond(
    resources: &plain_rs::infrastructure::host::resources::NativeResources,
    path: &str,
    stop: tokio::sync::watch::Receiver<bool>,
    headers: &HeaderMap,
) -> Result<Response, StatusCode> {
    if headers.contains_key(header::RANGE) {
        let input = resources
            .read_file(path, stop)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
        return Ok(Source::Native(input)
            .respond("application/octet-stream", headers)
            .await);
    }
    use plain_rs::infrastructure::host::resources::ResourceError;
    let reader = resources
        .read_native(path, stop)
        .await
        .map_err(|error| match error {
            ResourceError::Busy => StatusCode::TOO_MANY_REQUESTS,
            ResourceError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ResourceError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            ResourceError::Closed => StatusCode::BAD_GATEWAY,
        })?;
    let stream = futures_util::stream::try_unfold(reader, |mut reader| async move {
        reader
            .read()
            .await
            .map(|bytes| bytes.map(|bytes| (bytes, reader)))
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::ConnectionAborted, error))
    });
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        plain_libs::axum::body::Body::from_stream(stream),
    )
        .into_response())
}
