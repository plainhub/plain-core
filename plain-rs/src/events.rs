pub const MEDIA_LIBRARY_UPDATED: &str = "MEDIA_LIBRARY_UPDATED";
pub const MEDIA_SCAN_PROGRESS: &str = "MEDIA_SCAN_PROGRESS";
pub const WS_PAIRING_SUCCESS: &str = "PAIRING_SUCCESS";

#[derive(Clone, Debug)]
pub struct WsEvent {
    host_origin: bool,
    binary_permit: Option<std::sync::Arc<tokio::sync::OwnedSemaphorePermit>>,
    pub event_type: &'static str,
    pub payload: String,
    pub binary_payload: Option<Vec<u8>>,
    /// Delivery target: `Some(cid)` delivers only to the socket whose cid
    /// matches (per-client media/file-task/DLNA progress); `None`
    /// broadcasts to every connected socket.
    pub target_cid: Option<String>,
}

pub const HOST_TEXT_TYPES: &[&str] = &[
    "PAIRING_REQUEST_REPORTED",
    "PAIRING_RESPONSE_REPORTED",
    "PAIRING_CANCEL_REPORTED",
    "NOTIFICATION_REPORTED",
    "AUDIO_PROGRESS_REPORTED",
    "AUDIO_STARTED_REPORTED",
    "AUDIO_COMPLETED_REPORTED",
    "DLNA_POSITION_REPORTED",
    "DLNA_PLAYBACK_REPORTED",
    "DLNA_SEEK_CONSUMED_REPORTED",
    "DLNA_STOPPED_REPORTED",
    "IMAGE_CATALOG_CHANGED",
    "NETWORK_INTERFACES_CHANGED",
    "BLE_ADVERTISEMENT_REPORTED",
    "BLE_SCAN_STATE_REPORTED",
    "BLE_ADVERTISING_FACTS_REPORTED",
    "SCREEN_MIRRORING",
    "SCREEN_MIRROR_AUDIO_GRANTED",
    "SCREEN_MIRROR_VIDEO_CODEC",
    "SMS_PROVIDER_CHANGED",
    "SMS_PART_SENT_REPORTED",
    "MMS_SEND_RESULT",
    "CLIPBOARD_REPORTED",
    "PERMISSIONS_UPDATED",
];
pub const HOST_BINARY_TYPES: &[&str] = &["SCREEN_MIRROR_VIDEO", "SCREEN_MIRROR_AUDIO"];

impl WsEvent {
    pub fn into_public(self) -> Option<Self> {
        if self.is_internal() {
            return None;
        }
        if self.event_type != WS_PAIRING_SUCCESS {
            return Some(self);
        }
        #[cfg(feature = "chat")]
        {
            let mut event = self;
            if event.binary_payload.is_some() {
                return None;
            }
            #[derive(serde::Deserialize, serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct PublicPairingSuccess {
                device_id: String,
                device_name: String,
                ip: String,
            }
            let payload: PublicPairingSuccess = serde_json::from_str(&event.payload).ok()?;
            event.payload = serde_json::to_string(&payload).ok()?;
            Some(event)
        }
        #[cfg(not(feature = "chat"))]
        {
            None
        }
    }

    pub fn is_internal(&self) -> bool {
        matches!(
            self.event_type,
            "MDNS_UPDATED"
                | "PEER_CONNECTIONS_UPDATED"
                | "IMAGE_INDEX_UPDATED"
                | "IMAGE_MODELS_UPDATED"
                | "PREFS_UPDATED"
                | "NEARBY_DEVICES_UPDATED"
        )
    }

    pub fn is_host_event(&self) -> bool {
        self.host_origin
    }

    pub fn host_text(event_type: &str, payload: String) -> Option<Self> {
        let event_type = *HOST_TEXT_TYPES.iter().find(|&&kind| kind == event_type)?;
        let mut event = Self::broadcast(event_type, payload);
        event.host_origin = true;
        Some(event)
    }

    pub fn host_binary(event_type: &str, payload: Vec<u8>) -> Option<Self> {
        let event_type = *HOST_BINARY_TYPES.iter().find(|&&kind| kind == event_type)?;
        let mut event = Self::broadcast_binary(event_type, payload);
        event.host_origin = true;
        Some(event)
    }

    pub fn with_binary_permit(mut self, permit: tokio::sync::OwnedSemaphorePermit) -> Self {
        self.binary_permit = Some(std::sync::Arc::new(permit));
        self
    }
    pub fn broadcast(event_type: &'static str, payload: String) -> Self {
        Self {
            host_origin: false,
            binary_permit: None,
            event_type,
            payload,
            binary_payload: None,
            target_cid: None,
        }
    }

    pub fn targeted(event_type: &'static str, payload: String, cid: &str) -> Self {
        Self {
            host_origin: false,
            binary_permit: None,
            event_type,
            payload,
            binary_payload: None,
            target_cid: Some(cid.to_string()),
        }
    }

    pub fn broadcast_binary(event_type: &'static str, payload: Vec<u8>) -> Self {
        Self {
            host_origin: false,
            binary_permit: None,
            event_type,
            payload: String::new(),
            binary_payload: Some(payload),
            target_cid: None,
        }
    }
}

pub const WS_BOOKMARK_UPDATED: &'static str = "BOOKMARK_UPDATED";

pub const WS_POMODORO_ACTION: &'static str = "POMODORO_ACTION";

pub const WS_IMAGE_EDITOR_UPDATE: &'static str = "IMAGE_EDITOR_UPDATE";

#[cfg(test)]
#[path = "../tests/unit/events.rs"]
mod tests;

pub const WS_DEVICE_NAME_UPDATED: &str = "DEVICE_NAME_UPDATED";

pub const WS_NEARBY_DEVICE_UNREACHABLE: &str = "NEARBY_DEVICE_UNREACHABLE";

pub const WS_NEARBY_DISCOVERY_STARTED: &str = "NEARBY_DISCOVERY_STARTED";

pub const WS_NEARBY_DISCOVERY_STOPPED: &str = "NEARBY_DISCOVERY_STOPPED";

pub const WS_UPLOAD_MERGE_RESULT: &str = "UPLOAD_MERGE_RESULT";
