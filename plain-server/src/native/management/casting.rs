use super::NativeManagement;
use plain_rs::services::casting::receiver::control;
use plain_rs::services::casting::receiver::control::Request;
use serde_json::Value;

impl NativeManagement {
    async fn receiver(&self, request: Request) -> Result<Value, String> {
        self.state.native_client_id()?;
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = stop.changed() => Err("Core stopped".into()),
            result = control::execute(&self.state, request) => result,
        }
    }

    pub async fn dlna_receiver(&self) -> Result<Value, String> {
        let mut snapshot = self.receiver(Request::Snapshot {}).await?;
        snapshot["enabled"] = Value::Bool(plain_rs::prefs::dlna::enabled(&self.state.prefs));
        snapshot["rules"] = self.receiver(Request::Rules {}).await?;
        Ok(snapshot)
    }

    pub async fn retry_dlna_receiver(&self) -> Result<Value, String> {
        self.receiver(Request::Retry {}).await
    }

    pub async fn remove_dlna_rule(&self, ip: String, allowed: bool) -> Result<Value, String> {
        self.receiver(Request::RemoveRule { ip, allowed }).await
    }

    pub async fn accept_dlna_cast(&self, remember: bool) -> Result<Value, String> {
        self.receiver(Request::Accept { remember }).await
    }

    pub async fn reject_dlna_cast(&self, remember: bool) -> Result<Value, String> {
        self.receiver(Request::Reject { remember }).await
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/native/management/casting.rs"]
mod tests;
