use super::NativeManagement;

impl NativeManagement {
    pub async fn pairing_code(&self) -> Result<serde_json::Value, String> {
        let code = plain_rs::features::services::discovery::pairing_qr::display(&self.state)
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_value(code).map_err(|error| error.to_string())
    }

    pub fn is_pairing_request_current(
        &self,
        id: String,
        signature: String,
    ) -> Result<bool, String> {
        self.state.native_client_id()?;
        Ok(self
            .state
            .pairing
            .incoming_target(&id, &signature)
            .is_some())
    }

    pub async fn logout_peer(&self, id: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        let state = self.state.clone();
        tokio::task::spawn_blocking(move || {
            let _commit = state.pairing_runtime.commit.lock().unwrap();
            state.pairing_runtime.attempts.cancel_peer(&state, &id);
            state.db.logout_peer(&id)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub async fn reconnect_peers(&self) -> Result<bool, String> {
        self.state.native_client_id()?;
        plain_rs::features::services::peers::peer_status_controller::Controller::new(
            self.state.core.clone(),
        )
        .execute(plain_rs::features::services::peers::peer_status_controller::Action::Reconnect)
        .await?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/native/management/peers.rs"]
mod tests;
