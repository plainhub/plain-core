use anyhow::{Result, ensure};
use plain_rs::runtime::Runtime as CoreRuntime;
use serde::Deserialize;
use serde_json::json;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Facts {
    lease: uuid::Uuid,
    available: bool,
    aware_running: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Advertisement {
    lease: uuid::Uuid,
    bytes: Option<[u8; 9]>,
}

#[derive(Default)]
struct State {
    owner: Option<u64>,
    latest_generation: u64,
    dirty: bool,
    revision: u64,
    facts: Option<Facts>,
    applied: Option<Advertisement>,
}

#[derive(Default)]
pub struct Runtime {
    state: Mutex<State>,
    operation: tokio::sync::Mutex<()>,
}

impl Runtime {
    pub fn connect(&self, generation: u64) {
        let mut state = self.state.lock().unwrap();
        if generation <= state.latest_generation {
            return;
        }
        state.latest_generation = generation;
        state.owner = Some(generation);
        state.dirty = true;
        state.facts = None;
        state.applied = None;
    }

    pub fn close(&self, generation: u64) {
        let mut state = self.state.lock().unwrap();
        if state.owner == Some(generation) {
            state.owner = None;
            state.facts = None;
            state.applied = None;
        }
    }

    fn current(&self, state: &CoreRuntime, generation: u64) -> Result<()> {
        ensure!(!*state.stop.borrow(), "BLE core stopped");
        ensure!(state.host.is_current(generation), "BLE host replaced");
        ensure!(
            self.state.lock().unwrap().owner == Some(generation),
            "BLE advertising owner replaced"
        );
        Ok(())
    }

    pub async fn report(&self, state: &CoreRuntime, generation: u64, facts: Facts) -> Result<()> {
        self.current(state, generation)?;
        {
            let mut runtime = self.state.lock().unwrap();
            ensure!(
                runtime.owner == Some(generation),
                "BLE advertising owner replaced"
            );
            if runtime.facts == Some(facts) && !runtime.dirty {
                return Ok(());
            }
            runtime.facts = Some(facts);
            runtime.dirty = true;
        }
        self.reconcile(state, generation).await
    }

    pub async fn reconcile(&self, state: &CoreRuntime, generation: u64) -> Result<()> {
        let _operation = self.operation.lock().await;
        self.current(state, generation)?;
        let facts = {
            let mut runtime = self.state.lock().unwrap();
            ensure!(
                runtime.owner == Some(generation),
                "BLE advertising owner replaced"
            );
            let Some(facts) = runtime.facts else {
                return Ok(());
            };
            runtime.dirty = true;
            facts
        };
        let supported = if facts.available {
            plain_rs::services::capabilities::snapshot(&state.host)
                .await?
                .iter()
                .any(|value| value == "BLE_DISCOVERY")
        } else {
            false
        };
        self.current(state, generation)?;
        let bytes = if facts.available && supported {
            let id = state
                .prefs
                .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
                .unwrap_or_default();
            let aware_supported = state.host.startup().wifi_aware_supported;
            Some(plain_rs::discovery::advertisement::ble(
                &id,
                aware_supported,
                aware_supported && facts.aware_running,
            )?)
        } else {
            None
        };
        let advertisement = Advertisement {
            lease: facts.lease,
            bytes,
        };
        let revision = {
            let mut runtime = self.state.lock().unwrap();
            ensure!(
                runtime.owner == Some(generation),
                "BLE advertising owner replaced"
            );
            if runtime.facts != Some(facts) {
                return Ok(());
            }
            if runtime.applied.as_ref() == Some(&advertisement)
                || runtime.applied.is_none() && advertisement.bytes.is_none()
            {
                runtime.dirty = false;
                return Ok(());
            }
            runtime.revision = runtime
                .revision
                .checked_add(1)
                .filter(|revision| *revision <= i64::MAX as u64)
                .ok_or_else(|| anyhow::anyhow!("BLE advertising revision exhausted"))?;
            runtime.revision
        };
        let bytes = advertisement
            .bytes
            .as_ref()
            .map(|bytes| plain_rs::utils::base64::base64_encode(bytes))
            .unwrap_or_default();
        let accepted = state
            .host
            .call(
                "bleAdvertising",
                json!({"active":advertisement.bytes.is_some(),"bytes":bytes,"revision":revision,"lease":advertisement.lease}),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        self.current(state, generation)?;
        ensure!(accepted == json!(true), "BLE advertising command rejected");
        let mut runtime = self.state.lock().unwrap();
        ensure!(
            runtime.owner == Some(generation),
            "BLE advertising owner replaced"
        );
        runtime.applied = Some(advertisement);
        runtime.dirty = runtime.facts != Some(facts);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/discovery/ble_advertising.rs"]
mod tests;
