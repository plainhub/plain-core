use crate::platform::apple_device::{
    CF_NUMBER_SINT64_TYPE, CFArrayCreate, CFBooleanRef, CFDictionaryGetValueIfPresent,
    CFDictionaryRef, CFNumberGetValue, CFStringRef, kCFAllocatorDefault, kCFBooleanTrue,
    kCFTypeArrayCallBacks,
};
// Take a look at the license at the top of the repository in the LICENSE file.

use crate::platform::apple_device::{OwnedCF, cf_file_url};
use crate::system_metrics::Disk;
use crate::system_metrics::sys::ffi;

use libc::c_void;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr;

unsafe fn get_list(container: &mut Vec<Disk>) {
    container.clear();

    let raw_disks = {
        let count = libc::getfsstat(ptr::null_mut(), 0, libc::MNT_NOWAIT);
        if count < 1 {
            return;
        }
        let Some(bufsize) = (count as usize)
            .checked_mul(std::mem::size_of::<libc::statfs>())
            .and_then(|bytes| libc::c_int::try_from(bytes).ok())
        else {
            sysinfo_debug!("Mounted filesystem buffer exceeds native size limit");
            return;
        };
        let mut disks = vec![std::mem::zeroed::<libc::statfs>(); count as usize];
        let count = libc::getfsstat(disks.as_mut_ptr(), bufsize, libc::MNT_NOWAIT);

        if count < 1 {
            return;
        }

        disks.truncate(count as usize);

        disks
    };

    // Create a list of properties about the disk that we want to fetch.
    let requested_properties = match build_requested_properties(&[
        ffi::kCFURLVolumeIsEjectableKey,
        ffi::kCFURLVolumeIsRemovableKey,
        ffi::kCFURLVolumeIsInternalKey,
        ffi::kCFURLVolumeTotalCapacityKey,
        ffi::kCFURLVolumeAvailableCapacityForImportantUsageKey,
        ffi::kCFURLVolumeAvailableCapacityKey,
        ffi::kCFURLVolumeNameKey,
        ffi::kCFURLVolumeIsBrowsableKey,
        ffi::kCFURLVolumeIsLocalKey,
    ]) {
        Some(properties) => properties,
        None => {
            sysinfo_debug!("failed to create volume key list");
            return;
        }
    };

    for c_disk in raw_disks {
        let mount_bytes = c_disk
            .f_mntonname
            .iter()
            .take_while(|&&byte| byte != 0)
            .map(|&byte| byte as u8)
            .collect();
        let mount_point = PathBuf::from(OsString::from_vec(mount_bytes));
        let volume_url = match cf_file_url(&mount_point) {
            Some(url) => url,
            None => {
                sysinfo_debug!("getfsstat returned incompatible paths");
                continue;
            }
        };

        let prop_dict = match get_disk_properties(&volume_url, &requested_properties) {
            Some(props) => props,
            None => continue,
        };

        // Future note: There is a difference between `kCFURLVolumeIsBrowsableKey` and the
        // `kCFURLEnumeratorSkipInvisibles` option of `CFURLEnumeratorOptions`. Specifically,
        // the first one considers the writable `Data`(`/System/Volumes/Data`) partition to be
        // browsable, while it is classified as "invisible" by CoreFoundation's volume emumerator.
        let browsable =
            get_bool_value(prop_dict.as_ptr(), ffi::kCFURLVolumeIsBrowsableKey).unwrap_or_default();

        // Do not return invisible "disks". Most of the time, these are APFS snapshots, hidden
        // system volumes, etc. Browsable is defined to be visible in the system's UI like Finder,
        // disk utility, system information, etc.
        //
        // To avoid seemingly duplicating many disks and creating an inaccurate view of the system's
        // resources, these are skipped entirely.
        if !browsable {
            continue;
        }

        let local_only =
            get_bool_value(prop_dict.as_ptr(), ffi::kCFURLVolumeIsLocalKey).unwrap_or(true);

        // Skip any drive that is not locally attached to the system.
        //
        // This includes items like SMB mounts, and matches the other platform's behavior.
        if !local_only {
            continue;
        }

        if let Some(disk) = new_disk(mount_point, c_disk, &prop_dict) {
            container.push(disk);
        }
    }
}

type RetainedCFArray = OwnedCF<std::ffi::c_void>;
type RetainedCFDictionary = OwnedCF<std::ffi::c_void>;

unsafe fn build_requested_properties(properties: &[CFStringRef]) -> Option<RetainedCFArray> {
    OwnedCF::from_owned(CFArrayCreate(
        ptr::null_mut(),
        properties.as_ptr() as *const *const c_void,
        properties.len() as _,
        &kCFTypeArrayCallBacks,
    ))
}

fn get_disk_properties(
    volume_url: &OwnedCF,
    requested_properties: &RetainedCFArray,
) -> Option<RetainedCFDictionary> {
    unsafe {
        OwnedCF::from_owned(ffi::CFURLCopyResourcePropertiesForKeys(
            volume_url.as_ptr().cast(),
            requested_properties.as_ptr(),
            ptr::null_mut(),
        ))
    }
}

fn get_available_volume_space(disk_props: &RetainedCFDictionary) -> u64 {
    // We prefer `AvailableCapacityForImportantUsage` over `AvailableCapacity` because
    // it takes more of the system's properties into account, like the trash, system-managed caches,
    // etc. It generally also returns higher values too, because of the above, so it's a more
    // accurate representation of what the system _could_ still use.
    unsafe {
        get_int_value(
            disk_props.as_ptr(),
            ffi::kCFURLVolumeAvailableCapacityForImportantUsageKey,
        )
        .filter(|bytes| *bytes != 0)
        .or_else(|| get_int_value(disk_props.as_ptr(), ffi::kCFURLVolumeAvailableCapacityKey))
    }
    .unwrap_or_default() as u64
}

unsafe fn get_dict_value<T>(
    dict: CFDictionaryRef,
    key: CFStringRef,
    read: impl FnOnce(*const c_void) -> Option<T>,
) -> Option<T> {
    let mut value = std::ptr::null();
    if CFDictionaryGetValueIfPresent(dict, key.cast(), &mut value) != 0 {
        read(value)
    } else {
        None
    }
}

unsafe fn get_str_value(dict: CFDictionaryRef, key: CFStringRef) -> Option<String> {
    get_dict_value(dict, key, |value| {
        crate::platform::apple_device::cf_string(value)
    })
}

unsafe fn get_bool_value(dict: CFDictionaryRef, key: CFStringRef) -> Option<bool> {
    get_dict_value(dict, key, |v| Some(v as CFBooleanRef == kCFBooleanTrue))
}

unsafe fn get_int_value(dict: CFDictionaryRef, key: CFStringRef) -> Option<i64> {
    get_dict_value(dict, key, |v| {
        let mut val: i64 = 0;
        if CFNumberGetValue(
            v.cast(),
            CF_NUMBER_SINT64_TYPE,
            &mut val as *mut i64 as *mut c_void,
        ) != 0
        {
            Some(val)
        } else {
            None
        }
    })
}

unsafe fn new_disk(
    mount_point: PathBuf,
    c_disk: libc::statfs,
    disk_props: &RetainedCFDictionary,
) -> Option<Disk> {
    // Note: Since we requested these properties from the system, we don't expect
    // these property retrievals to fail.

    let name = get_str_value(disk_props.as_ptr(), ffi::kCFURLVolumeNameKey).map(OsString::from)?;

    let is_removable = {
        let ejectable = get_bool_value(disk_props.as_ptr(), ffi::kCFURLVolumeIsEjectableKey)
            .unwrap_or_default();

        let removable = get_bool_value(disk_props.as_ptr(), ffi::kCFURLVolumeIsRemovableKey)
            .unwrap_or_default();

        let is_removable = ejectable || removable;

        if is_removable {
            is_removable
        } else {
            // If neither `ejectable` or `removable` return `true`, fallback to checking
            // if the disk is attached to the internal system.
            let internal = get_bool_value(disk_props.as_ptr(), ffi::kCFURLVolumeIsInternalKey)
                .unwrap_or_default();

            !internal
        }
    };

    let total_space = get_int_value(disk_props.as_ptr(), ffi::kCFURLVolumeTotalCapacityKey)? as u64;

    let available_space = get_available_volume_space(disk_props);

    let file_system = {
        let len = c_disk
            .f_fstypename
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(c_disk.f_fstypename.len());
        OsString::from_vec(
            c_disk.f_fstypename[..len]
                .iter()
                .map(|c| *c as u8)
                .collect(),
        )
    };

    Some(Disk {
        name,
        file_system,
        mount_point,
        total_bytes: total_space,
        available_bytes: available_space,
        removable: is_removable,
    })
}

unsafe fn with_autorelease<T, F: FnOnce() -> T>(call: F) -> T {
    // NB: This struct exists to help prevent memory leaking if `call` were to panic.
    // Otherwise, the call to `objc_autoreleasePoolPop` would never be made as the stack unwinds.
    // `Drop` destructors for existing types on the stack are run during unwinding, so we can
    // ensure the autorelease pool is drained by using a RAII pattern here.
    struct DrainPool {
        ctx: *mut c_void,
    }

    impl Drop for DrainPool {
        fn drop(&mut self) {
            // SAFETY: We have not manipulated `pool_ctx` since it was received from a corresponding
            // pool push call.
            unsafe { ffi::objc_autoreleasePoolPop(self.ctx) }
        }
    }

    // SAFETY: Creating a new pool is safe in any context. They can be arbitrarily nested
    // as long as pool objects are not used in deeper layers, but we only have one and don't
    // allow it to leave this scope.
    let _pool_ctx = DrainPool {
        ctx: unsafe { ffi::objc_autoreleasePoolPush() },
    };
    call()
    // Pool is drained here before returning
}

pub(crate) fn disks() -> Vec<Disk> {
    let mut disks = Vec::new();
    unsafe {
        with_autorelease(|| get_list(&mut disks));
    }
    disks
}
