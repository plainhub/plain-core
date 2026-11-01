use super::*;

#[test]
fn windows_prefix_structs_match_the_documented_64_bit_abi() {
    if cfg!(target_pointer_width = "64") {
        assert_eq!(std::mem::offset_of!(Adapter, name), 16);
        assert_eq!(std::mem::offset_of!(Adapter, unicast), 24);
        assert_eq!(std::mem::offset_of!(Adapter, if_type), 100);
        assert_eq!(std::mem::offset_of!(Adapter, oper_status), 104);
        assert_eq!(std::mem::offset_of!(Adapter, ipv6_index), 108);
        assert_eq!(std::mem::offset_of!(Unicast, socket), 16);
        assert_eq!(std::mem::offset_of!(Unicast, prefix_length), 56);
    }
}

#[test]
fn windows_adapter_chain_retains_types_flags_prefixes_and_ipv6_scope() {
    let name = c"test-adapter";
    let mut v4 = [0u8; 16];
    v4[..2].copy_from_slice(&2u16.to_ne_bytes());
    v4[4..8].copy_from_slice(&[192, 0, 2, 1]);
    let mut v6 = [0u8; 28];
    v6[..2].copy_from_slice(&23u16.to_ne_bytes());
    v6[8..24].copy_from_slice(&"fe80::1".parse::<Ipv6Addr>().unwrap().octets());
    v6[24..28].copy_from_slice(&7u32.to_ne_bytes());
    let mut second: Unicast = unsafe { std::mem::zeroed() };
    second.socket = SocketAddress {
        address: v6.as_ptr(),
        length: v6.len() as i32,
    };
    second.prefix_length = 64;
    let mut first: Unicast = unsafe { std::mem::zeroed() };
    first.socket = SocketAddress {
        address: v4.as_ptr(),
        length: v4.len() as i32,
    };
    first.prefix_length = 24;
    first.next = std::ptr::addr_of!(second);
    let mut adapter: Adapter = unsafe { std::mem::zeroed() };
    adapter.alignment = (7u64 << 32) | std::mem::size_of::<Adapter>() as u64;
    adapter.name = name.as_ptr();
    adapter.unicast = std::ptr::addr_of!(first);
    adapter.if_type = 71;
    adapter.oper_status = 1;
    let rows = unsafe { collect(&adapter) };
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.name, "test-adapter");
    assert_eq!(row.index, 7);
    assert_eq!(row.if_type, InterfaceType::Wireless80211);
    assert!(row.up);
    assert!(!row.loopback);
    assert!(!row.tunnel);
    assert_eq!(row.ipv4[0].ip, Ipv4Addr::new(192, 0, 2, 1));
    assert_eq!(row.ipv4[0].netmask, Some(Ipv4Addr::new(255, 255, 255, 0)));
    assert_eq!(row.ipv6[0].index, 7);
    first.prefix_length = 255;
    adapter.unicast = std::ptr::addr_of!(first);
    assert_eq!(unsafe { collect(&adapter) }[0].ipv4[0].netmask, None);
    adapter.if_type = 131;
    assert!(unsafe { collect(&adapter) }[0].tunnel);
}

#[test]
fn windows_decoder_preserves_empty_adapters_and_rejects_truncated_addresses() {
    let mut adapter: Adapter = unsafe { std::mem::zeroed() };
    adapter.name = c"test-empty".as_ptr();
    adapter.if_type = 24;
    adapter.ipv6_index = 9;
    assert!(unsafe { collect(std::ptr::null()) }.is_empty());
    let row = &unsafe { collect(&adapter) }[0];
    assert!(!row.up);
    assert!(row.loopback);
    assert!(row.ipv4.is_empty());
    let mut bytes = [0u8; 28];
    bytes[..2].copy_from_slice(&23u16.to_ne_bytes());
    bytes[8..24].copy_from_slice(&"2001:db8::1".parse::<Ipv6Addr>().unwrap().octets());
    let mut address: Unicast = unsafe { std::mem::zeroed() };
    address.socket.address = bytes.as_ptr();
    address.socket.length = 27;
    adapter.unicast = std::ptr::addr_of!(address);
    assert!(unsafe { collect(&adapter) }[0].ipv6.is_empty());
    address.socket.length = 28;
    adapter.unicast = std::ptr::addr_of!(address);
    assert_eq!(unsafe { collect(&adapter) }[0].ipv6[0].index, 9);
    address.socket.length = -1;
    adapter.unicast = std::ptr::addr_of!(address);
    assert!(unsafe { collect(&adapter) }[0].ipv6.is_empty());
}
