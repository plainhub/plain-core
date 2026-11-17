pub(crate) mod resource;
pub(crate) mod upload;

use plain_libs::protocols::http_client;

pub(crate) fn request_error(error: plain_libs::anyhow::Error) -> String {
    use plain_rs::infrastructure::error_code::ErrorCode;
    if error
        .downcast_ref::<http_client::Error>()
        .is_some_and(http_client::Error::is_timeout)
    {
        return ErrorCode::ConnectionTimeout.to_string();
    }
    error.to_string()
}

#[cfg(test)]
#[path = "../../tests/plain-server/unit/managed_peers.rs"]
mod tests;
