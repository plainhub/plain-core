use crate::{http::ConnectionScheme, server::ServerState};
use axum::{Extension, Json, extract::State, http::StatusCode};
use plain_rs::features::prefs::security::{PasswordSetupError, setup_password_hash};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetupRequest {
    password_hash: String,
}

pub(crate) async fn setup(
    State(state): State<ServerState>,
    Extension(scheme): Extension<ConnectionScheme>,
    Json(request): Json<SetupRequest>,
) -> StatusCode {
    if scheme.0 != "https" || !state.prefs.get_or("remote_access", true) {
        return StatusCode::FORBIDDEN;
    }
    let prefs = state.prefs.clone();
    match tokio::task::spawn_blocking(move || setup_password_hash(&prefs, &request.password_hash))
        .await
    {
        Err(error) => {
            log::error!("Password setup failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
        Ok(result) => match result {
            Ok(true) => StatusCode::NO_CONTENT,
            Ok(false) => StatusCode::CONFLICT,
            Err(PasswordSetupError::InvalidPassword) => StatusCode::BAD_REQUEST,
            Err(PasswordSetupError::Preferences(error)) => {
                log::error!("Password setup failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        },
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/auth/setup.rs"]
mod tests;
