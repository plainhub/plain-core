use super::InterfaceType;
use std::{collections::HashMap, ffi::c_void, io};

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
    fn CFRelease(value: *const c_void);
}

#[link(name = "SystemConfiguration", kind = "framework")]
unsafe extern "C" {
    fn SCNetworkInterfaceCopyAll() -> *const c_void;
    fn SCNetworkInterfaceGetBSDName(interface: *const c_void) -> *const c_void;
    fn SCNetworkInterfaceGetInterfaceType(interface: *const c_void) -> *const c_void;
    static kSCNetworkInterfaceTypeIEEE80211: *const c_void;
    static kSCNetworkInterfaceTypeEthernet: *const c_void;
    static kSCNetworkInterfaceTypeWWAN: *const c_void;
    static kSCNetworkInterfaceTypePPP: *const c_void;
}

pub(super) fn interface_types() -> io::Result<HashMap<String, InterfaceType>> {
    let array = unsafe { SCNetworkInterfaceCopyAll() };
    if array.is_null() {
        return Err(io::Error::other(
            "SystemConfiguration network interfaces unavailable",
        ));
    }
    let mut result = HashMap::new();
    for index in 0..unsafe { CFArrayGetCount(array) } {
        let interface = unsafe { CFArrayGetValueAtIndex(array, index) };
        let name = unsafe { SCNetworkInterfaceGetBSDName(interface) };
        let kind = unsafe { SCNetworkInterfaceGetInterfaceType(interface) };
        if name.is_null() || kind.is_null() {
            continue;
        }
        let Some(name) = (unsafe { crate::platform::apple_device::cf_string(name) }) else {
            continue;
        };
        let mut if_type = InterfaceType::Unknown;
        for (native, value) in unsafe {
            [
                (
                    kSCNetworkInterfaceTypeIEEE80211,
                    InterfaceType::Wireless80211,
                ),
                (kSCNetworkInterfaceTypeEthernet, InterfaceType::Ethernet),
                (kSCNetworkInterfaceTypeWWAN, InterfaceType::Cellular),
                (kSCNetworkInterfaceTypePPP, InterfaceType::Ppp),
            ]
        } {
            if unsafe { CFEqual(kind, native) } != 0 {
                if_type = value;
                break;
            }
        }
        result.insert(name, if_type);
    }
    unsafe { CFRelease(array) };
    Ok(result)
}
