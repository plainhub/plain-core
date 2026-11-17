use plain_rs::features::peers::error::PeerConnectionErrorCode;
pub(crate) mod resource;
pub(crate) mod upload;

use plain_libs::protocols::http_client;

pub(crate) fn request_error(error: plain_libs::anyhow::Error) -> String {
        if error
        .downcast_ref::<http_client::Error>()
        .is_some_and(http_client::Error::is_timeout)
    {
        return PeerConnectionErrorCode::ConnectionTimeout.to_string();
    }
    error.to_string()
}

#[cfg(test)]
#[path = "../../tests/plain-server/unit/managed_peers.rs"]
mod tests;
