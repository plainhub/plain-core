use crate::features::discovery::nearby_devices::Device;
use crate::infrastructure::db::chat_store;
use crate::infrastructure::events::WsEvent;
use crate::infrastructure::transport::lan;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::serde_json::json;
#[derive(Clone)]
pub struct Context {
    pub db: std::sync::Arc<crate::infrastructure::db::Db>,
    pub prefs: std::sync::Arc<crate::features::prefs::Prefs>,
    pub devices: std::sync::Arc<crate::features::discovery::nearby_devices::Devices>,
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
    pub fn seen(
        &self,
        device: Device,
        visible: bool,
        resident: bool,
    ) -> plain_libs::anyhow::Result<bool> {
        self.seen_with_interfaces(device, visible, resident, lan::local_interfaces)
    }

    fn seen_with_interfaces(
        &self,
        device: Device,
        visible: bool,
        resident: bool,
        interfaces: impl FnOnce() -> Vec<lan::Interface>,
    ) -> plain_libs::anyhow::Result<bool> {
        if device.id
            == self
                .prefs
                .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
                .unwrap_or_default()
        {
            return Ok(false);
        }
        device.validate()?;
        if resident {
            chat_store::nearby::save(&self.db, &device.cache())?;
            if chat_store::peers::discovered(
                &self.db,
                &device.id,
                &lan::best(&device.ips, &interfaces()),
                device.port,
                &device.name,
                device.device_type,
            )?
            .is_some()
            {
                let _ = self.events.send(WsEvent::broadcast(
                    crate::features::chat::events::WS_PEER_UPDATED,
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
            self.devices.observe(device)?;
        }
        Ok(true)
    }
}
pub fn snapshot(
    db: &crate::infrastructure::db::Db,
    devices: &crate::features::discovery::nearby_devices::Devices,
) -> plain_libs::anyhow::Result<plain_libs::serde_json::Value> {
    let paired: std::collections::HashSet<_> = chat_store::peers::all(db)?
        .into_iter()
        .filter(|p| p.is_paired())
        .map(|p| p.id)
        .collect();
    let snapshot = devices.snapshot();
    let revision = snapshot.revision;
    let interfaces = lan::local_interfaces();
    let mut rows = Vec::new();
    for (device, best_ip) in snapshot.visible_devices(&interfaces) {
        let status = if paired.contains(&device.id) {
            crate::features::chat::enums::PeerStatus::Paired
        } else {
            crate::features::chat::enums::PeerStatus::Unpaired
        };
        let mut row = plain_libs::serde_json::to_value(device)?;
        row["bestIp"] = json!(best_ip);
        row["status"] = json!(status);
        rows.push(row);
    }
    Ok(json!({"revision": revision, "devices": rows}))
}
pub async fn sweep(state: &CoreRuntime) -> plain_libs::anyhow::Result<()> {
    let (lan, non_lan): (Vec<_>, Vec<_>) = state
        .nearby_devices
        .stale(&state.db)?
        .into_iter()
        .partition(|probe| {
            probe
                .device
                .discovery_methods
                .iter()
                .any(|method| method.is_lan())
        });
    let interfaces = if lan.is_empty() {
        Vec::new()
    } else {
        lan::local_interfaces()
    };
    for chunk in lan.chunks(4) {
        if !state.nearby_devices.active() {
            return Ok(());
        }
        let interfaces = &interfaces;
        let results = futures_util::future::join_all(chunk.iter().map(|p| async move {
            let ip = lan::best(&p.device.ips, interfaces);
            let alive = !ip.is_empty()
                && crate::features::discovery::nearby_http::probe(&ip, p.device.port)
                    .await
                    .unwrap_or(false);
            (p, alive)
        }))
        .await;
        for (probe, alive) in results {
            state.nearby_devices.verified(&state.db, probe, alive)?;
        }
    }
    if !non_lan.is_empty() && state.nearby_devices.active() {
        let facts = state
            .host
            .call("nearbyScanFacts", json!({}))
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
        if facts.get("paused").and_then(plain_libs::serde_json::Value::as_bool) == Some(false) {
            for probe in &non_lan {
                state.nearby_devices.verified(&state.db, probe, false)?;
            }
        }
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
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/nearby_devices.rs"]
mod tests;
