use crate::host::bridge::Host;
use plain_rs::platform::network::{Interface, InterfaceType};

pub(crate) async fn ipv4() -> Result<String, String> {
    let value = lan_ipv4().await?;
    if value.is_empty() {
        return Err("No LAN address available for casting".into());
    }
    let address: std::net::Ipv4Addr = value.parse().map_err(|_| "Invalid IPv4 reading")?;
    if address.is_unspecified() || address.is_loopback() {
        return Err("No LAN address available for casting".into());
    }
    Ok(address.to_string())
}

pub(crate) async fn lan_ipv4_addresses() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| {
        let interfaces =
            plain_rs::platform::network::interfaces().map_err(|error| error.to_string())?;
        Ok(select_lan_ipv4_addresses(&interfaces))
    })
    .await
    .map_err(|error| error.to_string())?
}

pub(crate) async fn lan_ipv4() -> Result<String, String> {
    Ok(lan_ipv4_addresses()
        .await?
        .into_iter()
        .next()
        .unwrap_or_default())
}

fn select_lan_ipv4_addresses(interfaces: &[Interface]) -> Vec<String> {
    let mut addresses = interfaces
        .iter()
        .filter(|interface| {
            interface.up
                && !interface.loopback
                && !interface.tunnel
                && !matches!(
                    interface.if_type,
                    InterfaceType::Tunnel | InterfaceType::Ppp
                )
        })
        .flat_map(|interface| {
            let priority = match interface.if_type {
                InterfaceType::Wireless80211 => 0,
                InterfaceType::Ethernet => 1,
                _ => 2,
            };
            interface
                .ipv4
                .iter()
                .map(move |network| (priority, network.ip))
        })
        .filter(|(_, address)| {
            !address.is_unspecified()
                && !address.is_loopback()
                && !address.is_link_local()
                && !address.is_multicast()
                && !address.is_broadcast()
        })
        .collect::<Vec<_>>();
    addresses.sort_by_key(|(priority, _)| *priority);
    let mut seen = std::collections::HashSet::new();
    addresses
        .into_iter()
        .filter_map(|(_, address)| seen.insert(address).then(|| address.to_string()))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum NetworkType {
    Wifi,
    Cellular,
    Ethernet,
    None,
    Other,
}

pub(crate) async fn network_type(host: &Host) -> Result<NetworkType, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        serde_json::from_value(
            host.call("systemNetworkType", serde_json::json!({}))
                .await?,
        )
        .map_err(|error| error.to_string())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = host;
        tokio::task::spawn_blocking(|| {
            classify_network(
                plain_rs::platform::network::default_interface()
                    .map(|interface| interface.map(|interface| interface.if_type)),
            )
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn classify_network(kind: std::io::Result<Option<InterfaceType>>) -> Result<NetworkType, String> {
    Ok(match kind.map_err(|error| error.to_string())? {
        Some(InterfaceType::Wireless80211) => NetworkType::Wifi,
        Some(InterfaceType::Ethernet) => NetworkType::Ethernet,
        Some(InterfaceType::Cellular) => NetworkType::Cellular,
        Some(_) => NetworkType::Other,
        None => NetworkType::None,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/platform/network.rs"]
mod tests;
