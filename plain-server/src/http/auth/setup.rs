use crate::{http::ConnectionScheme, server::ServerState};
use axum::{Extension, Json, extract::State, http::StatusCode};
use plain_rs::features::prefs::security::{PasswordSetupError, setup_password};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetupRequest {
    password: String,
}

pub(crate) async fn setup(
    State(state): State<ServerState>,
    Extension(scheme): Extension<ConnectionScheme>,
    Json(request): Json<SetupRequest>,
) -> StatusCode {
    if scheme.0 != "https" || !state.prefs.get_or("desktop_access", true) {
        return StatusCode::FORBIDDEN;
    }
    match setup_password(&state.prefs, &request.password) {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::CONFLICT,
        Err(PasswordSetupError::InvalidPassword) => StatusCode::BAD_REQUEST,
        Err(PasswordSetupError::Preferences(error)) => {
            log::error!("Password setup failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/http/auth/setup.rs"]
mod tests;
