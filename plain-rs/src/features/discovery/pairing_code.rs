use super::nearby_devices::Device;
use crate::libs::utils::query::percent_decode;

#[derive(Debug, PartialEq, serde::Deserialize)]
pub struct Payload {
    pub id: String,
    pub name: String,
    pub ips: Vec<String>,
    pub port: u16,
}

pub fn format(value: &Payload) -> Option<String> {
    let encode = crate::libs::utils::query::url_encode;
    let code = format!(
        "plainapp://pair?v=1&id={}&name={}&ips={}&port={}",
        encode(&value.id),
        encode(&value.name),
        encode(&value.ips.join(",")),
        value.port
    );
    parse(&code).map(|_| code)
}

pub fn parse(code: &str) -> Option<Payload> {
    let query = code.strip_prefix("plainapp://pair?")?;
    let mut value = Payload {
        id: String::new(),
        name: String::new(),
        ips: Vec::new(),
        port: 0,
    };
    for field in query.split('&') {
        let Some((key, raw)) = field.split_once('=') else {
            continue;
        };
        let decoded = percent_decode(raw);
        match key {
            "id" => value.id = decoded,
            "name" => value.name = decoded,
            "ips" => value.ips.extend(
                decoded
                    .split(',')
                    .filter(|ip| !ip.trim().is_empty())
                    .map(str::to_owned),
            ),
            "port" => value.port = decoded.parse().unwrap_or(0),
            _ => {}
        }
    }
    if value.id.trim().is_empty()
        || value.name.trim().is_empty()
        || value.ips.is_empty()
        || value.port == 0
    {
        return None;
    }
    Some(value)
}

impl Payload {
    pub fn into_device(self) -> Device {
        Device {
            id: self.id,
            name: self.name,
            ips: self.ips,
            port: self.port,
            device_type: "COMPUTER".into(),
            version: String::new(),
            platform: String::new(),
            last_seen_at: crate::libs::db::now_iso(),
            discovery_methods: vec!["LAN".into(), "QR".into()],
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/discovery/pairing_code.rs"]
mod tests;
