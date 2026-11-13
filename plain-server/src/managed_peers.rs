pub(crate) mod resource;
pub(crate) mod upload;

use plain_libs::protocols::http_client;

pub(crate) fn request_error(error: plain_libs::anyhow::Error) -> String {
    use plain_rs::features::services::peers::management_http::AuthorizationError;
    if error
        .downcast_ref::<http_client::Error>()
        .is_some_and(http_client::Error::is_timeout)
    {
        return "connection_timeout".into();
    }
    match error.downcast_ref::<AuthorizationError>() {
        Some(AuthorizationError::Session) => "unauthorized".into(),
        Some(AuthorizationError::Permission) => "remote_access_disabled".into(),
        None => error.to_string(),
    }
}

#[cfg(test)]
#[path = "../../tests/plain-server/unit/managed_peers.rs"]
mod tests;
