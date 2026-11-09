pub mod control;
pub mod http_router;
pub mod lifecycle;
pub mod receiver_engine;
pub mod renderer_state;
pub mod reports;
pub mod soap_handler;
pub mod ssdp_messages;
pub mod types;
pub mod xml_templates;

/// Whether a persisted sender list (`ip|name` entries, see
pub fn senders_contain_ip(list: &[String], ip: &str) -> bool {
    crate::features::prefs::dlna::senders_contain_ip(list, ip)
}

pub fn is_receiver_path(method: &str, path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    matches!(
        (method, path),
        (
            "GET" | "HEAD",
            "/dlna/description.xml"
                | "/dlna/av-transport/scpd.xml"
                | "/dlna/rendering-control/scpd.xml"
        ) | (
            "POST",
            "/dlna/av-transport/control" | "/dlna/rendering-control/control"
        ) | (
            "SUBSCRIBE" | "UNSUBSCRIBE",
            "/dlna/av-transport/event" | "/dlna/rendering-control/event"
        )
    )
}
