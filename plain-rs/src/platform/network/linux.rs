use super::InterfaceType;
use std::path::Path;

pub(super) fn interface_type(name: &str) -> InterfaceType {
    read_type(Path::new("/sys/class/net"), name)
}

fn read_type(root: &Path, name: &str) -> InterfaceType {
    let path = root.join(name);
    let properties = std::fs::read_to_string(path.join("uevent")).unwrap_or_default();
    if properties.lines().any(|line| line.trim() == "DEVTYPE=wlan")
        || path.join("wireless").exists()
        || path.join("phy80211").exists()
    {
        return InterfaceType::Wireless80211;
    }
    if properties.lines().any(|line| line.trim() == "DEVTYPE=wwan")
        || ["wwan", "wwp", "rmnet", "ccmni"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
    {
        return InterfaceType::Cellular;
    }
    match std::fs::read_to_string(path.join("type"))
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
    {
        Some(1) => InterfaceType::Ethernet,
        Some(512) => InterfaceType::Ppp,
        Some(768 | 769 | 776 | 778 | 823) => InterfaceType::Tunnel,
        _ => InterfaceType::Unknown,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/network/linux.rs"]
mod tests;
