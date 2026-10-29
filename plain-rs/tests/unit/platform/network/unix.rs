use super::*;
use std::{cell::Cell, ffi::c_char};

#[test]
fn native_records_are_grouped_once_and_keep_ipv4_masks_and_ipv6_scope() {
    let name = c"test-network";
    let mut v4: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    v4.sin_family = libc::AF_INET as _;
    v4.sin_addr.s_addr = u32::from_ne_bytes([192, 0, 2, 9]);
    let mut mask: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    mask.sin_family = libc::AF_INET as _;
    mask.sin_addr.s_addr = u32::from_ne_bytes([255, 255, 255, 0]);
    let mut v6: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
    v6.sin6_family = libc::AF_INET6 as _;
    v6.sin6_addr.s6_addr = "2001:db8::9".parse::<Ipv6Addr>().unwrap().octets();
    v6.sin6_scope_id = 17;
    let mut second: libc::ifaddrs = unsafe { std::mem::zeroed() };
    second.ifa_name = name.as_ptr() as *mut c_char;
    second.ifa_addr = std::ptr::addr_of_mut!(v6).cast();
    let mut first: libc::ifaddrs = unsafe { std::mem::zeroed() };
    first.ifa_name = name.as_ptr() as *mut c_char;
    first.ifa_flags = (libc::IFF_UP | libc::IFF_POINTOPOINT) as u32;
    first.ifa_addr = std::ptr::addr_of_mut!(v4).cast();
    first.ifa_netmask = std::ptr::addr_of_mut!(mask).cast();
    first.ifa_next = std::ptr::addr_of_mut!(second);
    let kinds = Cell::new(0);
    let indices = Cell::new(0);
    let rows = unsafe {
        collect(
            std::ptr::addr_of_mut!(first),
            |_| {
                kinds.set(kinds.get() + 1);
                super::super::InterfaceType::Tunnel
            },
            |_| {
                indices.set(indices.get() + 1);
                7
            },
        )
    };
    assert_eq!(kinds.get(), 1);
    assert_eq!(indices.get(), 1);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.up);
    assert!(row.tunnel);
    assert!(!row.loopback);
    assert_eq!(row.if_type, super::super::InterfaceType::Tunnel);
    assert_eq!(row.ipv4[0].ip, Ipv4Addr::new(192, 0, 2, 9));
    assert_eq!(row.ipv4[0].netmask, Some(Ipv4Addr::new(255, 255, 255, 0)));
    assert_eq!(row.ipv6[0].index, 17);
    v6.sin6_scope_id = 0;
    second.ifa_addr = std::ptr::addr_of_mut!(v6).cast();
    first.ifa_next = std::ptr::addr_of_mut!(second);
    first.ifa_netmask = std::ptr::null_mut();
    let rows = unsafe { collect(std::ptr::addr_of_mut!(first), |_| Default::default(), |_| 7) };
    assert_eq!(rows[0].ipv4[0].netmask, None);
    assert_eq!(rows[0].ipv6[0].index, 7);
}

#[test]
fn null_names_and_non_ip_records_do_not_create_fabricated_addresses() {
    let mut row: libc::ifaddrs = unsafe { std::mem::zeroed() };
    assert!(
        unsafe {
            collect(
                std::ptr::addr_of_mut!(row),
                |_| panic!("missing name"),
                |_| panic!("missing name"),
            )
        }
        .is_empty()
    );
    row.ifa_name = c"loopback-test".as_ptr() as *mut c_char;
    row.ifa_flags = (libc::IFF_UP | libc::IFF_LOOPBACK) as u32;
    let rows = unsafe { collect(std::ptr::addr_of_mut!(row), |_| Default::default(), |_| 1) };
    assert_eq!(rows.len(), 1);
    assert!(rows[0].loopback);
    assert!(rows[0].ipv4.is_empty());
    assert!(rows[0].ipv6.is_empty());
    let mut address: libc::sockaddr = unsafe { std::mem::zeroed() };
    address.sa_family = 127;
    row.ifa_addr = std::ptr::addr_of_mut!(address);
    assert!(
        unsafe { collect(std::ptr::addr_of_mut!(row), |_| Default::default(), |_| 1) }[0]
            .ipv4
            .is_empty()
    );
}
