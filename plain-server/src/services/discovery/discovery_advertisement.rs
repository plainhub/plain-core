use crate::assembly::server::ServerState;
use anyhow::Result;
use plain_rs::discovery::advertisement;
use plain_rs::discovery::advertisement::Facts;
use serde_json::{Value, json};
pub(crate) enum Request {
    Reply {},
    Mdns {},
}
pub(crate) async fn execute(state: &ServerState, request: Request) -> Result<Value> {
    let id = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .unwrap_or_default();
    let settings = plain_rs::prefs::settings::snapshot(&state.prefs)?;
    let name = settings.device_name;
    let port = state.public_ports().await.map(|ports| ports.1).unwrap_or(0);
    let reply = advertisement::reply(&id, &name, port, facts(state).await?)?;
    match request {
        Request::Reply {} => Ok(serde_json::to_value(reply)?),
        Request::Mdns {} => Ok(serde_json::to_value(advertisement::mdns(
            &reply,
            &plain_rs::prefs::ensure_mdns_hostname(&state.prefs),
        )?)?),
    }
}
pub(crate) fn device_type(state: &ServerState) -> plain_rs::chat::enums::DeviceType {
    state.host.startup().device_type
}

async fn aware_flags(state: &ServerState) -> Result<(bool, bool)> {
    let supported = state.host.startup().wifi_aware_supported;
    if !supported {
        return Ok((false, false));
    }
    let sample = state
        .host
        .call("systemProperties", json!({"names":["wifiAwareRunning"]}))
        .await
        .map_err(anyhow::Error::msg)?;
    let running = sample["wifiAwareRunning"]
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("Invalid Wi-Fi Aware state reading"))?;
    Ok((true, running))
}
async fn facts(state: &ServerState) -> Result<Facts> {
    let name = state.host.startup().name.clone();
    let device_type = device_type(state);
    let version = state.host.startup().app_version.clone();
    let (addresses, (aware_supported, aware_running)) = tokio::try_join!(
        async {
            crate::platform::network::lan_ipv4_addresses(&state.host)
                .await
                .map_err(anyhow::Error::msg)
        },
        aware_flags(state),
    )?;
    Ok(Facts {
        name,
        device_type,
        version,
        platform: std::env::consts::OS.to_uppercase(),
        ips: addresses,
        aware_supported,
        aware_running,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/services/discovery/discovery_advertisement.rs"]
mod tests;
