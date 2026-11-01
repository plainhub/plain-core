use super::*;

#[test]
fn the_lists_are_well_formed_or_empty() {
    for interface in list() {
        assert!(!interface.ip.is_loopback());
        assert!(!interface.name.is_empty());
    }
    for interface in list_v6() {
        assert!(!interface.ip.is_loopback());
        assert!(!interface.name.is_empty());
        assert!(!is_link_local(&interface.ip));
    }
}

#[cfg(unix)]
#[test]
fn a_v6_index_matches_what_the_kernel_reports_for_that_name() {
    for interface in interfaces().1 {
        let name = std::ffi::CString::new(interface.name).unwrap();
        assert_eq!(interface.index, unsafe {
            libc::if_nametoindex(name.as_ptr())
        });
    }
}
