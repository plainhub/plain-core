use super::NativeManagement;

impl NativeManagement {
    pub async fn pairing_code(&self) -> Result<String, String> {
        self.state.native_client_id()?;
        let reply = plain_rs::services::discovery::discovery_advertisement::execute(
            &self.state,
            plain_rs::services::discovery::discovery_advertisement::Request::Reply {},
        )
        .await
        .map_err(|error| error.to_string())?;
        let mut payload: plain_rs::discovery::pairing_code::Payload =
            serde_json::from_value(reply).map_err(|error| error.to_string())?;
        payload.ips.retain(|address| {
            address
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| !address.is_loopback() && !address.is_unspecified())
        });
        plain_rs::discovery::pairing_code::format(&payload)
            .ok_or_else(|| "Reachable pairing address and active listener required".to_owned())
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
        plain_rs::services::peers::peer_status_controller::Controller::new(self.state.core.clone())
            .execute(plain_rs::services::peers::peer_status_controller::Action::Reconnect)
            .await?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/native/management/peers.rs"]
mod tests;
