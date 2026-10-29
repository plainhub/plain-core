use std::net::{Ipv4Addr, Ipv6Addr};

/// One IPv4 interface entry. `netmask` is `None` if the kernel reported no
/// netmask (uncommon but legal — e.g. point-to-point links).
#[derive(Debug, Clone)]
pub struct Ifv4 {
    pub name: String,
    pub ip: Ipv4Addr,
    pub netmask: Option<Ipv4Addr>,
}

/// One IPv6 interface entry. `index` is the kernel's interface index, which
/// multicast joins need; it is `0` only when the name cannot be resolved.
#[derive(Debug, Clone)]
pub struct Ifv6 {
    pub name: String,
    pub ip: Ipv6Addr,
    pub index: u32,
}

/// Every up, non-loopback IPv4 interface.
pub fn list() -> Vec<Ifv4> {
    interfaces().0
}

/// Every up, non-loopback IPv6 interface that is routable off-link.
///
/// Link-local (`fe80::/10`) addresses are left out, which is what the
/// `if-addrs` crate did by default and what the mDNS caller was written
/// against: they are only meaningful together with their interface, and
/// feeding them in would make the responder announce itself on every VPN
/// and AWDL interface the machine happens to have.
pub fn list_v6() -> Vec<Ifv6> {
    interfaces()
        .1
        .into_iter()
        .filter(|iface| !is_link_local(&iface.ip))
        .collect()
}

fn is_link_local(ip: &Ipv6Addr) -> bool {
    ip.segments()[0] & 0xffc0 == 0xfe80
}

fn interfaces() -> (Vec<Ifv4>, Vec<Ifv6>) {
    let interfaces = match crate::platform::network::interfaces() {
        Ok(interfaces) => interfaces,
        Err(error) => {
            log::warn!("[ifaddr] interface enumeration failed: {error}");
            return (Vec::new(), Vec::new());
        }
    };
    let (mut v4, mut v6) = (Vec::new(), Vec::new());
    for interface in interfaces
        .into_iter()
        .filter(|interface| interface.up && !interface.loopback)
    {
        for address in interface.ipv4 {
            v4.push(Ifv4 {
                name: interface.name.clone(),
                ip: address.ip,
                netmask: address.netmask,
            });
        }
        for address in interface.ipv6 {
            v6.push(Ifv6 {
                name: interface.name.clone(),
                ip: address.ip,
                index: address.index,
            });
        }
    }
    (v4, v6)
}

#[cfg(test)]
#[path = "../../tests/unit/utils/ifaddr.rs"]
mod tests;
