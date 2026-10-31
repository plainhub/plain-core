use plain_rs::db::chat_store;
use plain_rs::discovery::nearby_devices::Device;
use plain_rs::events::WsEvent;
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::json;
pub const EVENT_UPDATED: &'static str = "NEARBY_DEVICES_UPDATED";

fn publish(state: &CoreRuntime) {
    let _ = state.events.send(WsEvent::broadcast(
        EVENT_UPDATED,
        json!({"revision":state.nearby_devices.snapshot().revision}).to_string(),
    ));
}
#[derive(Clone)]
pub struct Context {
    pub db: std::sync::Arc<plain_rs::db::Db>,
    pub prefs: std::sync::Arc<plain_rs::prefs::Prefs>,
    pub devices: std::sync::Arc<plain_rs::discovery::nearby_devices::Devices>,
    pub events: tokio::sync::broadcast::Sender<WsEvent>,
}
impl From<&CoreRuntime> for Context {
    fn from(state: &CoreRuntime) -> Self {
        Self {
            db: state.db.clone(),
            prefs: state.prefs.clone(),
            devices: state.nearby_devices.clone(),
            events: state.events.clone(),
        }
    }
}
impl Context {
    pub fn seen(&self, device: Device, visible: bool, resident: bool) -> anyhow::Result<bool> {
        if device.id
            == self
                .prefs
                .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
                .unwrap_or_default()
        {
            return Ok(false);
        }
        device.validate()?;
        if resident {
            chat_store::nearby::save(&self.db, &device.cache())?;
            let kind = serde_json::from_value(json!(device.device_type))
                .unwrap_or(plain_rs::chat::enums::DeviceType::Other);
            if chat_store::peers::discovered(
                &self.db,
                &device.id,
                &device.ips,
                device.port,
                &device.name,
                kind,
            )?
            .is_some()
            {
                let _ = self.events.send(WsEvent::broadcast(
                    plain_rs::chat::events::WS_PEER_UPDATED,
                    json!({"id":device.id}).to_string(),
                ));
            }
        }
        if visible
            || self
                .devices
                .snapshot()
                .devices
                .iter()
                .any(|d| d.id == device.id)
        {
            let id = device.id.clone();
            let emit = self.devices.observe(device, visible)?;
            let _ = self.events.send(WsEvent::broadcast(
                EVENT_UPDATED,
                json!({"revision":self.devices.snapshot().revision}).to_string(),
            ));
            if visible && emit {
                let snapshot = snapshot(&self.db, &self.devices)?;
                if let Some(device) = snapshot["devices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|device| device["id"] == id)
                {
                    let _ = self.events.send(WsEvent::broadcast(
                        plain_rs::chat::events::WS_NEARBY_DEVICE_FOUND,
                        device.to_string(),
                    ));
                }
            }
        }
        Ok(true)
    }
}
pub fn snapshot(
    db: &plain_rs::db::Db,
    devices: &plain_rs::discovery::nearby_devices::Devices,
) -> anyhow::Result<serde_json::Value> {
    let paired: std::collections::HashSet<_> = chat_store::peers::all(db)?
        .into_iter()
        .filter(|p| p.is_paired())
        .map(|p| p.id)
        .collect();
    let mut snapshot = serde_json::to_value(devices.snapshot())?;
    let interfaces = plain_rs::transport::lan::local_interfaces();
    for device in snapshot["devices"].as_array_mut().unwrap() {
        let ips: Vec<String> = serde_json::from_value(device["ips"].clone())?;
        device["bestIp"] = json!(plain_rs::transport::lan::best(&ips, &interfaces));
        device["status"] = json!(if paired.contains(device["id"].as_str().unwrap()) {
            "PAIRED"
        } else {
            "UNPAIRED"
        });
    }
    Ok(snapshot)
}
pub async fn sweep(state: &CoreRuntime) -> anyhow::Result<()> {
    let stale = state.nearby_devices.stale(&state.db)?;
    if stale.is_empty() {
        return Ok(());
    }
    let facts = state
        .host
        .call("nearbyScanFacts", json!({}))
        .await
        .map_err(anyhow::Error::msg)?;
    if facts.get("paused").and_then(serde_json::Value::as_bool) != Some(false) {
        return Ok(());
    }
    let interfaces = plain_rs::transport::lan::local_interfaces();
    let mut changed = false;
    for chunk in stale.chunks(4) {
        if !state.nearby_devices.active() {
            break;
        }
        let interfaces = &interfaces;
        let results = futures_util::future::join_all(chunk.iter().map(|p| async move {
            let ip = plain_rs::transport::lan::best(&p.device.ips, &interfaces);
            let alive = p.device.discovery_methods.iter().any(|v| v == "LAN")
                && !ip.is_empty()
                && plain_rs::discovery::nearby_http::probe(&ip, p.device.port)
                    .await
                    .unwrap_or(false);
            (p, alive)
        }))
        .await;
        for (probe, alive) in results {
            changed |= state.nearby_devices.verified(&state.db, probe, alive)?;
        }
    }
    if changed {
        publish(state);
    }
    Ok(())
}
pub fn start(state: CoreRuntime) {
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(20));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! { _=stop.changed()=>break, _=timer.tick()=>{tokio::select! {_=stop.changed()=>break, _=sweep(&state)=>{}}} }
        }
    });
}

#[cfg(test)]
#[path = "../../../tests/unit/services/discovery/nearby_devices.rs"]
mod tests;
