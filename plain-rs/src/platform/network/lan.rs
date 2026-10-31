use plain_rs::platform::network::Interface;
use plain_rs::platform::network::InterfaceType;
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkInterfaceFacts {
    name: String,
    network_type: NetworkType,
    vpn: bool,
}

pub(crate) fn apply_interface_facts(
    mut interfaces: Vec<Interface>,
    facts: Vec<NetworkInterfaceFacts>,
) -> Vec<Interface> {
    for interface in &mut interfaces {
        let matches = facts.iter().filter(|fact| fact.name == interface.name);
        for fact in matches {
            interface.tunnel |= fact.vpn;
            interface.if_type = match fact.network_type {
                NetworkType::Wifi => InterfaceType::Wireless80211,
                NetworkType::Ethernet => InterfaceType::Ethernet,
                NetworkType::Cellular => InterfaceType::Cellular,
                NetworkType::None | NetworkType::Other => InterfaceType::Unknown,
            };
        }
    }
    interfaces
}

pub fn select_lan_ipv4_addresses(interfaces: &[Interface]) -> Vec<String> {
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
pub enum NetworkType {
    Wifi,
    Cellular,
    Ethernet,
    None,
    Other,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn classify_network(
    kind: std::io::Result<Option<InterfaceType>>,
) -> Result<NetworkType, String> {
    Ok(match kind.map_err(|error| error.to_string())? {
        Some(InterfaceType::Wireless80211) => NetworkType::Wifi,
        Some(InterfaceType::Ethernet) => NetworkType::Ethernet,
        Some(InterfaceType::Cellular) => NetworkType::Cellular,
        Some(_) => NetworkType::Other,
        None => NetworkType::None,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/network/lan.rs"]
mod tests;
