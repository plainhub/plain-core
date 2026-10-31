use std::ffi::{c_char, c_void};

#[repr(C)]
pub(super) struct SocketAddress {
    pub(super) address: *const u8,
    pub(super) length: i32,
}

#[repr(C)]
pub(super) struct Unicast {
    pub(super) alignment: u64,
    pub(super) next: *const Self,
    pub(super) socket: SocketAddress,
    pub(super) prefix_origin: i32,
    pub(super) suffix_origin: i32,
    pub(super) dad_state: i32,
    pub(super) valid_lifetime: u32,
    pub(super) preferred_lifetime: u32,
    pub(super) lease_lifetime: u32,
    pub(super) prefix_length: u8,
}

#[repr(C)]
pub(super) struct Adapter {
    pub(super) alignment: u64,
    pub(super) next: *const Self,
    pub(super) name: *const c_char,
    pub(super) unicast: *const Unicast,
    pub(super) anycast: *const c_void,
    pub(super) multicast: *const c_void,
    pub(super) dns_servers: *const c_void,
    pub(super) dns_suffix: *const u16,
    pub(super) description: *const u16,
    pub(super) friendly_name: *const u16,
    pub(super) physical_address: [u8; 8],
    pub(super) physical_address_length: u32,
    pub(super) flags: u32,
    pub(super) mtu: u32,
    pub(super) if_type: u32,
    pub(super) oper_status: i32,
    pub(super) ipv6_index: u32,
}
