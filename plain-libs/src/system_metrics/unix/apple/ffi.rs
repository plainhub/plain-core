// Take a look at the license at the top of the repository in the LICENSE file.

#[cfg(feature = "disk-metrics")]
use crate::platform::apple_device::{
    CFArrayRef, CFDictionaryRef, CFErrorRef, CFStringRef, CFURLRef,
};

#[cfg(feature = "disk-metrics")]
use std::ffi::c_void;

#[cfg(feature = "disk-metrics")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFURLCopyResourcePropertiesForKeys(
        url: CFURLRef,
        keys: CFArrayRef,
        error: *mut CFErrorRef,
    ) -> CFDictionaryRef;

    pub static kCFURLVolumeIsEjectableKey: CFStringRef;
    pub static kCFURLVolumeIsRemovableKey: CFStringRef;
    pub static kCFURLVolumeAvailableCapacityKey: CFStringRef;
    pub static kCFURLVolumeAvailableCapacityForImportantUsageKey: CFStringRef;
    pub static kCFURLVolumeTotalCapacityKey: CFStringRef;
    pub static kCFURLVolumeNameKey: CFStringRef;
    pub static kCFURLVolumeIsLocalKey: CFStringRef;
    pub static kCFURLVolumeIsInternalKey: CFStringRef;
    pub static kCFURLVolumeIsBrowsableKey: CFStringRef;
}

#[cfg(feature = "disk-metrics")]
#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    pub fn objc_autoreleasePoolPop(pool: *mut c_void);
    pub fn objc_autoreleasePoolPush() -> *mut c_void;
}
