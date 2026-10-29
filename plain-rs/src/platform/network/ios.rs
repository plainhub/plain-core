use super::InterfaceType;

pub(super) fn interface_type(name: &str) -> InterfaceType {
    if name == "en0" {
        InterfaceType::Wireless80211
    } else if name.starts_with("en") || name.starts_with("bridge") {
        InterfaceType::Ethernet
    } else if name.starts_with("utun") || name.starts_with("ipsec") {
        InterfaceType::Tunnel
    } else if name.starts_with("ppp") {
        InterfaceType::Ppp
    } else {
        InterfaceType::Unknown
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/network/ios.rs"]
mod tests;
