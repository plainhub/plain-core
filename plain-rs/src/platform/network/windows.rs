use super::{AddressV4, AddressV6, Interface, InterfaceType};
use std::{
    ffi::CStr,
    net::{Ipv4Addr, Ipv6Addr},
};
#[cfg(windows)]
use std::{ffi::c_void, io};

mod abi;
use abi::*;

#[cfg(windows)]
#[link(name = "iphlpapi")]
unsafe extern "system" {
    fn GetAdaptersAddresses(
        family: u32,
        flags: u32,
        reserved: *const c_void,
        adapters: *mut Adapter,
        length: *mut u32,
    ) -> u32;
}

#[cfg(windows)]
pub(super) fn interfaces() -> io::Result<Vec<Interface>> {
    let mut size = 0;
    let status =
        unsafe { GetAdaptersAddresses(0, 0x2e, std::ptr::null(), std::ptr::null_mut(), &mut size) };
    if status == 232 {
        return Ok(Vec::new());
    }
    if status != 111 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let mut storage = vec![0u64; (size as usize).div_ceil(8)];
    let head = storage.as_mut_ptr().cast::<Adapter>();
    let status = unsafe { GetAdaptersAddresses(0, 0x2e, std::ptr::null(), head, &mut size) };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(unsafe { collect(head) })
}

unsafe fn collect(head: *const Adapter) -> Vec<Interface> {
    let mut result = Vec::new();
    let mut current: *const Adapter = head;
    while let Some(adapter) = unsafe { current.as_ref() } {
        current = adapter.next;
        let index = (adapter.alignment >> 32) as u32;
        let mut interface = Interface {
            name: unsafe { CStr::from_ptr(adapter.name) }
                .to_string_lossy()
                .into_owned(),
            index: if index == 0 {
                adapter.ipv6_index
            } else {
                index
            },
            if_type: match adapter.if_type {
                6 | 26 | 62 | 69 | 117 => InterfaceType::Ethernet,
                71 => InterfaceType::Wireless80211,
                243 | 244 => InterfaceType::Cellular,
                131 => InterfaceType::Tunnel,
                23 => InterfaceType::Ppp,
                _ => InterfaceType::Unknown,
            },
            up: adapter.oper_status == 1,
            loopback: adapter.if_type == 24,
            tunnel: matches!(adapter.if_type, 23 | 131),
            ..Default::default()
        };
        let mut current = adapter.unicast;
        while let Some(address) = unsafe { current.as_ref() } {
            current = address.next;
            if address.socket.address.is_null() || address.socket.length < 2 {
                continue;
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(address.socket.address, address.socket.length as usize)
            };
            match u16::from_ne_bytes([bytes[0], bytes[1]]) {
                2 if bytes.len() >= 16 => interface.ipv4.push(AddressV4 {
                    ip: Ipv4Addr::new(bytes[4], bytes[5], bytes[6], bytes[7]),
                    netmask: (address.prefix_length <= 32).then(|| {
                        Ipv4Addr::from(
                            u32::MAX
                                .checked_shl(u32::from(32 - address.prefix_length))
                                .unwrap_or(0),
                        )
                    }),
                }),
                23 if bytes.len() >= 28 => interface.ipv6.push(AddressV6 {
                    ip: Ipv6Addr::from(<[u8; 16]>::try_from(&bytes[8..24]).unwrap()),
                    index: match u32::from_ne_bytes(bytes[24..28].try_into().unwrap()) {
                        0 => adapter.ipv6_index,
                        index => index,
                    },
                }),
                _ => {}
            }
        }
        result.push(interface);
    }
    result
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/network/windows.rs"]
mod tests;
