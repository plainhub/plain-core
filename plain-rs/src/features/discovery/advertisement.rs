use crate::features::chat::enums::DeviceType;
use crate::features::discovery::nearby_wire::DiscoverReply;
use crate::infrastructure::protocols::mdns::service_info::build_service_info;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::protocols::mdns::records::MdnsServiceInfo;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Facts {
    pub name: String,
    pub device_type: DeviceType,
    pub version: String,
    pub platform: String,
    pub ips: Vec<String>,
    pub aware_supported: bool,
    pub aware_running: bool,
}
pub fn reply(id: &str, name: &str, port: u16, facts: Facts) -> Result<DiscoverReply> {
    ensure!(!id.is_empty(), "Missing client identity");
    Ok(DiscoverReply {
        id: id.into(),
        name: if name.is_empty() {
            facts.name
        } else {
            name.into()
        },
        port,
        remote_access: None,
        signature_public_key: String::new(),
        device_type: facts.device_type,
        version: facts.version,
        platform: facts.platform,
        ips: facts.ips,
        aware_supported: facts.aware_supported,
        aware_running: facts.aware_running,
    })
}
pub fn mdns(reply: &DiscoverReply, hostname: &str) -> Result<MdnsServiceInfo> {
    ensure!(reply.port != 0, "Missing discovery HTTPS port");
    ensure!(!hostname.is_empty(), "Missing mDNS hostname");
    let mut service = build_service_info(
        &reply.name,
        hostname,
        reply.port,
        &reply.id,
        reply.device_type.as_str(),
        &reply.version,
        &reply.platform,
        reply.ips.clone(),
        reply.aware_supported,
        reply.aware_running,
    );
    service.txt_records.push(format!("pk={}", reply.signature_public_key));
    if let Some(enabled) = reply.remote_access {
        service
            .txt_records
            .push(format!("ra={}", u8::from(enabled)));
    }
    Ok(service)
}
pub fn decode_ble(service_uuids: &[String]) -> Option<String> {
    service_uuids
        .iter()
        .find_map(|value| crate::features::client_identity::parse(value).map(|id| id.to_string()))
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/discovery/advertisement.rs"]
mod tests;
