#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEventRow {
    pub id: String,
    pub r#type: crate::features::audit_events::EventType,
    pub data: crate::features::audit_events::EventData,
    pub client_id: Option<String>,
    pub client_name: Option<String>,
    pub client_ip: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub app_name: Option<String>,
    pub app_version: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditEventClient {
    pub client_id: String,
    pub client_name: String,
    pub client_ip: String,
    pub os_name: String,
    pub os_version: String,
    pub app_name: String,
    pub app_version: String,
}
