use crate::assembly::server::ServerState;
use crate::services::casting::sender::types::DiscoveredDevice;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Device {
    pub id: String,
    pub host_address: String,
    pub name: String,
    pub location: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Item {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub album_art: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub audio: bool,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub version: u64,
    #[serde(skip)]
    pub current_audio: bool,
    pub devices: Vec<Device>,
    pub current_device: Option<Device>,
    pub items: Vec<Item>,
    pub current_uri: String,
    pub playing: bool,
    pub progress_ms: i64,
    pub duration_ms: i64,
    pub supports_callback: bool,
    pub active: bool,
    pub sid: String,
}
#[derive(Default)]
pub(crate) struct Runtime {
    pub snapshot: Mutex<Snapshot>,
    delivery: Mutex<bool>,
    pub devices: Mutex<HashMap<String, DiscoveredDevice>>,
    pub scan: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub polling: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub renewal: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub scan_lease: Mutex<Option<MulticastLease>>,
    pub receiver_lease: Mutex<Option<MulticastLease>>,
    pub subscription_device: Mutex<Option<DiscoveredDevice>>,
    pub subscription_renew_after: Mutex<std::time::Duration>,
    pub callback_sequence: Mutex<Option<u32>>,
    pub receiver_operations: tokio::sync::Mutex<()>,
    pub operations: tokio::sync::Mutex<()>,
}

pub(crate) struct MulticastLease {
    id: String,
    generation: Option<u64>,
}
impl Runtime {
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().unwrap().clone()
    }
    fn host_snapshot(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self.snapshot()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("sid");
        object.remove("active");
        value
    }
    pub(crate) fn notify_snapshot(&self, host: &crate::host::bridge::Host) -> Result<(), String> {
        let _delivery = self.delivery.lock().unwrap();
        host.notify("dlnaSenderChanged", self.host_snapshot())
    }
    pub fn publish(&self, state: &ServerState) {
        let mut recovering = self.delivery.lock().unwrap();
        {
            let mut snapshot = self.snapshot.lock().unwrap();
            snapshot.version = snapshot.version.saturating_add(1);
        }
        let snapshot = self.host_snapshot();
        let _ = state.events.send(plain_rs::events::WsEvent::broadcast(
            "DLNA_SENDER_CHANGED", snapshot.to_string(),
        ));
        if let Err(error) = state.host.notify("dlnaSenderChanged", snapshot) {
            log::warn!("DLNA sender callback: {error}");
            if *recovering || !state.host.connected() { return; }
            *recovering = true;
            let state = state.clone();
            tokio::spawn(async move {
                let permit = state.host.reserve_callback().await;
                let mut recovering = state.cast.delivery.lock().unwrap();
                if let Ok(permit) = permit {
                    if let Err(error) = state.host.notify_reserved_callback("dlnaSenderChanged", state.cast.host_snapshot(), permit) {
                        log::warn!("DLNA sender recovery callback: {error}");
                    }
                }
                *recovering = false;
            });
        }
    }
    pub fn stop_tasks(&self) {
        for slot in [&self.scan, &self.polling, &self.renewal] {
            if let Some(task) = slot.lock().unwrap().take() {
                task.abort();
            }
        }
    }
    pub fn device(&self) -> Result<DiscoveredDevice, String> {
        let id = self
            .snapshot()
            .current_device
            .ok_or("No cast device selected")?
            .id;
        self.devices
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| "Cast device disappeared".into())
    }
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Command {
    StartScan,
    StopScan,
    Select {
        id: String,
    },
    Cast {
        item: Item,
    },
    Play,
    Pause,
    Seek {
        #[serde(rename = "positionMs")]
        position_ms: i64,
    },
    Stop,
    Exit,
    Add {
        item: Item,
    },
    Remove {
        path: String,
    },
    RemoveAt {
        index: usize,
    },
    Reorder {
        from: usize,
        to: usize,
    },
    Clear,
}
pub(crate) async fn run(state: &ServerState, command: Command) -> Result<Snapshot, String> {
    let runtime = state.cast.clone();
    let _guard = runtime.operations.lock().await;
    let result = execute(state, &runtime, command).await;
    runtime.publish(state);
    result?;
    Ok(runtime.snapshot())
}
pub(crate) async fn execute(
    state: &ServerState,
    runtime: &Arc<Runtime>,
    command: Command,
) -> Result<(), String> {
    use Command::*;
    match command {
        StartScan => {
            if runtime
                .scan
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|task| task.is_finished())
            {
                release_permission(state, &runtime.scan_lease).await;
                acquire_permission(state, &runtime.scan_lease).await?;
                crate::services::casting::dlna_sender_scan::start(state);
            }
        }
        StopScan => {
            if let Some(task) = runtime.scan.lock().unwrap().take() {
                task.abort();
            }
            release_permission(state, &runtime.scan_lease).await;
        }
        Select { id } => {
            let device = runtime
                .snapshot()
                .devices
                .into_iter()
                .find(|d| d.id == id)
                .ok_or("Unknown cast device")?;
            if runtime
                .snapshot()
                .current_device
                .as_ref()
                .is_some_and(|current| current.id != id)
            {
                crate::services::casting::dlna_sender_playback::end(state, false).await;
            }
            runtime.snapshot.lock().unwrap().current_device = Some(device);
        }
        Cast { item } => {
            crate::services::casting::dlna_sender_playback::cast(state, item, false).await?
        }
        Play | Pause => {
            let play = matches!(command, Play);
            let device = runtime.device()?;
            crate::services::casting::dlna_sender_playback::soap(
                state,
                &device,
                if play { "Play" } else { "Pause" },
                if play {
                    "<InstanceID>0</InstanceID><Speed>1</Speed>"
                } else {
                    "<InstanceID>0</InstanceID>"
                },
            )
            .await?;
            runtime.snapshot.lock().unwrap().playing = play;
        }
        Seek { position_ms } => {
            let device = runtime.device()?;
            let ms = position_ms.max(0);
            let seconds = ms / 1000;
            let target = format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            );
            crate::services::casting::dlna_sender_playback::soap(
                state,
                &device,
                "Seek",
                &format!(
                    "<InstanceID>0</InstanceID><Unit>REL_TIME</Unit><Target>{target}</Target>"
                ),
            )
            .await?;
            runtime.snapshot.lock().unwrap().progress_ms = ms;
        }
        Stop | Exit => {
            let stop = matches!(command, Stop);
            crate::services::casting::dlna_sender_playback::end(state, stop).await;
        }
        Add { item } => runtime.snapshot.lock().unwrap().items.push(item),
        Remove { path } => runtime
            .snapshot
            .lock()
            .unwrap()
            .items
            .retain(|item| item.path != path),
        RemoveAt { index } => {
            let mut s = runtime.snapshot.lock().unwrap();
            if index < s.items.len() {
                s.items.remove(index);
            }
        }
        Reorder { from, to } => {
            let mut s = runtime.snapshot.lock().unwrap();
            if from < s.items.len() && to < s.items.len() {
                let item = s.items.remove(from);
                s.items.insert(to, item);
            }
        }
        Clear => {
            let mut s = runtime.snapshot.lock().unwrap();
            s.items.clear();
            s.current_uri.clear();
            s.current_audio = false;
            s.playing = false;
            s.progress_ms = 0;
            s.duration_ms = 0;
            s.supports_callback = false;
        }
    }
    Ok(())
}

pub(crate) async fn acquire_permission(
    state: &ServerState,
    slot: &Mutex<Option<MulticastLease>>,
) -> Result<(), String> {
    let generation = state.host.generation();
    if slot
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|lease| lease.generation == generation)
    {
        return Ok(());
    }
    release_permission(state, slot).await;
    let lease = uuid::Uuid::new_v4().to_string();
    if state
        .host
        .call("mdnsMulticast", json!({"lease":lease,"acquire":true}))
        .await?
        .as_bool()
        != Some(true)
    {
        return Err("Multicast permission unavailable".into());
    }
    if state.host.generation() != generation {
        return Err("Host replaced while acquiring multicast permission".into());
    }
    *slot.lock().unwrap() = Some(MulticastLease {
        id: lease,
        generation,
    });
    Ok(())
}
pub(crate) async fn release_permission(state: &ServerState, slot: &Mutex<Option<MulticastLease>>) {
    let lease = slot.lock().unwrap().take();
    if let Some(lease) = lease {
        let _ = state
            .host
            .call("mdnsMulticast", json!({"lease":lease.id,"acquire":false}))
            .await;
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/casting/dlna_sender_runtime.rs"]
mod tests;
