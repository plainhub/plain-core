use super::InterfaceType;
use crate::platform::apple_device::{CFArrayGetCount, CFArrayGetValueAtIndex, OwnedCF, cf_string};
use std::{collections::HashMap, ffi::c_void, io};

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
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
    let array = unsafe { OwnedCF::from_owned(SCNetworkInterfaceCopyAll()) }
        .ok_or_else(|| io::Error::other("SystemConfiguration network interfaces unavailable"))?;
    let mut result = HashMap::new();
    for index in 0..unsafe { CFArrayGetCount(array.as_ptr()) } {
        let interface = unsafe { CFArrayGetValueAtIndex(array.as_ptr(), index) };
        let name = unsafe { SCNetworkInterfaceGetBSDName(interface) };
        let kind = unsafe { SCNetworkInterfaceGetInterfaceType(interface) };
        if name.is_null() || kind.is_null() {
            continue;
        }
        let Some(name) = (unsafe { cf_string(name) }) else {
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
    Ok(result)
}
