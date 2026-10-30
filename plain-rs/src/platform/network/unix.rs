#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
use super::InterfaceType;
use super::{AddressV4, AddressV6, Interface};
use std::{
    ffi::CStr,
    io,
    net::{Ipv4Addr, Ipv6Addr},
};

struct Addresses(*mut libc::ifaddrs);
impl Drop for Addresses {
    fn drop(&mut self) {
        unsafe { libc::freeifaddrs(self.0) };
    }
}

pub(super) fn interfaces() -> io::Result<Vec<Interface>> {
    let mut head = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let _addresses = Addresses(head);
    #[cfg(target_os = "macos")]
    let kinds = super::macos::interface_types()?;
    #[cfg(target_os = "android")]
    let kind = |_: &str| InterfaceType::Unknown;
    #[cfg(not(target_os = "android"))]
    let kind = |name: &str| {
        #[cfg(target_os = "macos")]
        return kinds.get(name).copied().unwrap_or_default();
        #[cfg(target_os = "linux")]
        return super::linux::interface_type(name);
        #[cfg(target_os = "ios")]
        return super::ios::interface_type(name);
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
        InterfaceType::Unknown
    };
    Ok(unsafe { collect(head, kind, |name| libc::if_nametoindex(name.as_ptr())) })
}

unsafe fn collect(
    head: *mut libc::ifaddrs,
    mut kind: impl FnMut(&str) -> super::InterfaceType,
    mut index_of: impl FnMut(&CStr) -> u32,
) -> Vec<Interface> {
    let mut interfaces: Vec<Interface> = Vec::new();
    let mut current = head;
    while let Some(row) = unsafe { current.as_ref() } {
        current = row.ifa_next;
        if row.ifa_name.is_null() {
            continue;
        }
        let name = unsafe { CStr::from_ptr(row.ifa_name) };
        let name_text = name.to_string_lossy();
        let index = if let Some(index) = interfaces.iter().position(|i| i.name == name_text) {
            index
        } else {
            let tunnel = row.ifa_flags & libc::IFF_POINTOPOINT as u32 != 0;
            interfaces.push(Interface {
                if_type: kind(&name_text),
                name: name_text.into_owned(),
                index: index_of(name),
                up: row.ifa_flags & libc::IFF_UP as u32 != 0,
                loopback: row.ifa_flags & libc::IFF_LOOPBACK as u32 != 0,
                tunnel,
                ..Default::default()
            });
            interfaces.len() - 1
        };
        if row.ifa_addr.is_null() {
            continue;
        }
        let interface = &mut interfaces[index];
        match i32::from(unsafe { (*row.ifa_addr).sa_family }) {
            libc::AF_INET => {
                let address = unsafe { &*row.ifa_addr.cast::<libc::sockaddr_in>() };
                let netmask = if row.ifa_netmask.is_null() {
                    None
                } else {
                    let mask = unsafe { &*row.ifa_netmask.cast::<libc::sockaddr_in>() };
                    Some(Ipv4Addr::from(mask.sin_addr.s_addr.to_ne_bytes()))
                };
                interface.ipv4.push(AddressV4 {
                    ip: Ipv4Addr::from(address.sin_addr.s_addr.to_ne_bytes()),
                    netmask,
                });
            }
            libc::AF_INET6 => {
                let address = unsafe { &*row.ifa_addr.cast::<libc::sockaddr_in6>() };
                interface.ipv6.push(AddressV6 {
                    ip: Ipv6Addr::from(address.sin6_addr.s6_addr),
                    index: if address.sin6_scope_id == 0 {
                        interface.index
                    } else {
                        address.sin6_scope_id
                    },
                });
            }
            _ => {}
        }
    }
    interfaces
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/network/unix.rs"]
mod tests;
