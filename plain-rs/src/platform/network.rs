use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket},
};

#[cfg(any(target_os = "ios", test))]
mod ios;
#[cfg(any(target_os = "linux", target_os = "android", test))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(any(windows, test))]
mod windows;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InterfaceType {
    Wireless80211,
    Ethernet,
    Cellular,
    Tunnel,
    Ppp,
    #[default]
    Unknown,
}

#[derive(Clone, Debug)]
pub struct AddressV4 {
    pub ip: Ipv4Addr,
    pub netmask: Option<Ipv4Addr>,
}

#[derive(Clone, Debug)]
pub struct AddressV6 {
    pub ip: Ipv6Addr,
    pub index: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Interface {
    pub name: String,
    pub index: u32,
    pub if_type: InterfaceType,
    pub ipv4: Vec<AddressV4>,
    pub ipv6: Vec<AddressV6>,
    pub up: bool,
    pub loopback: bool,
    pub tunnel: bool,
}

pub fn interfaces() -> io::Result<Vec<Interface>> {
    #[cfg(unix)]
    return unix::interfaces();
    #[cfg(windows)]
    return windows::interfaces();
    #[cfg(not(any(unix, windows)))]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Network interface enumeration unavailable",
    ))
}

pub fn default_interface() -> io::Result<Option<Interface>> {
    let interfaces = interfaces()?;
    let address = default_address(|bind, remote| {
        let socket = UdpSocket::bind(bind).ok()?;
        socket.connect(remote).ok()?;
        Some(socket.local_addr().ok()?.ip())
    });
    Ok(address.and_then(|address| {
        interfaces
            .into_iter()
            .find(|interface| owns(interface, address))
    }))
}

fn default_address(mut probe: impl FnMut(&str, &str) -> Option<IpAddr>) -> Option<IpAddr> {
    [
        ("0.0.0.0:0", "8.8.8.8:80"),
        ("[::]:0", "[2001:4860:4860::8888]:80"),
    ]
    .into_iter()
    .find_map(|(bind, remote)| probe(bind, remote))
}

fn owns(interface: &Interface, address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => interface.ipv4.iter().any(|entry| entry.ip == address),
        IpAddr::V6(address) => interface.ipv6.iter().any(|entry| entry.ip == address),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/platform/network.rs"]
mod tests;
