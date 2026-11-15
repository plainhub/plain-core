use plain_graphql::{ID, SimpleObject};

use plain_rs::infrastructure::types::Instant;
use plain_rs::infrastructure::types::Long;

#[derive(SimpleObject, Clone, Debug)]
pub struct Disk {
    pub id: ID,
    pub name: String,
    pub path: String,
    pub size_bytes: Long,
    pub removable: bool,
    pub model: Option<String>,
}

pub use plain_rs::features::audit_events::{EventData, EventType};

#[derive(SimpleObject, Clone, Debug)]
pub struct Event {
    pub id: ID,
    pub r#type: EventType,
    pub data: EventData,
    pub client_id: Option<ID>,
    pub client_name: Option<String>,
    pub client_ip: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub app_name: Option<String>,
    pub app_version: Option<String>,

    pub created_at: Instant,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct DlnaRenderer {
    pub udn: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model_name: Option<String>,
    pub location: String,
}
