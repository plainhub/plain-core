mod parser;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
#[derive(Clone, Debug)]
pub enum IpNet {
    V4(Ipv4Net),
    V6(Ipv6Net),
}
#[derive(Clone, Debug)]
pub struct Ipv4Net {
    network: u32,
    mask: u32,
}
#[derive(Clone, Debug)]
pub struct Ipv6Net {
    network: u128,
    mask: u128,
}
impl Ipv4Net {
    fn new(address: Ipv4Addr, prefix: u8) -> Result<Self, ()> {
        if prefix > 32 {
            return Err(());
        }
        let mask = u32::MAX.checked_shl(u32::from(32 - prefix)).unwrap_or(0);
        Ok(Self {
            network: u32::from(address) & mask,
            mask,
        })
    }
}
impl Ipv6Net {
    fn new(address: Ipv6Addr, prefix: u8) -> Result<Self, ()> {
        if prefix > 128 {
            return Err(());
        }
        let mask = u128::MAX.checked_shl(u32::from(128 - prefix)).unwrap_or(0);
        Ok(Self {
            network: u128::from(address) & mask,
            mask,
        })
    }
}
impl IpNet {
    pub fn contains(&self, address: &IpAddr) -> bool {
        match (self, address) {
            (Self::V4(net), IpAddr::V4(address)) => u32::from(*address) & net.mask == net.network,
            (Self::V6(net), IpAddr::V6(address)) => u128::from(*address) & net.mask == net.network,
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/ipnet.rs"]
mod tests;
