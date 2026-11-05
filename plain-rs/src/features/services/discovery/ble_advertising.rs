use crate::runtime::Runtime as CoreRuntime;
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Facts {
    lease: uuid::Uuid,
    available: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Advertisement {
    lease: uuid::Uuid,
    service_uuid: Option<String>,
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
            crate::features::services::capabilities::snapshot(&state.host)
                .await?
                .iter()
                .any(|value| value == "BLE")
        } else {
            false
        };
        self.current(state, generation)?;
        let service_uuid = if facts.available && supported {
            let id = state
                .prefs
                .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
                .unwrap_or_default();
            let uuid = crate::features::client_identity::parse(&id)
                .ok_or_else(|| anyhow::anyhow!("Invalid BLE client identity"))?;
            Some(uuid.to_string())
        } else {
            None
        };
        let advertisement = Advertisement {
            lease: facts.lease,
            service_uuid,
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
                || runtime.applied.is_none() && advertisement.service_uuid.is_none()
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
        let accepted = state
            .host
            .call(
                "bleAdvertising",
                json!({"serviceUuid":advertisement.service_uuid,"revision":revision,"lease":advertisement.lease}),
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
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/ble_advertising.rs"]
mod tests;
