// Take a look at the license at the top of the repository in the LICENSE file.

use crate::system_metrics::Disk;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use super::utils::c_buf_to_utf8_string;

// FIXME: if you want to get disk I/O usage:
// statfs.[f_syncwrites, f_asyncwrites, f_syncreads, f_asyncreads]

pub unsafe fn get_all_list(container: &mut Vec<Disk>) {
    container.clear();

    let mut fs_infos: *mut libc::statfs = std::ptr::null_mut();

    let count = libc::getmntinfo(&mut fs_infos, libc::MNT_WAIT);

    if count < 1 {
        return;
    }
    let mut vfs: libc::statvfs = std::mem::zeroed();
    let fs_infos: &[libc::statfs] = std::slice::from_raw_parts(fs_infos as _, count as _);

    for fs_info in fs_infos {
        if fs_info.f_mntfromname[0] == 0 || fs_info.f_mntonname[0] == 0 {
            // If we have missing information, no need to look any further...
            continue;
        }
        let fs_type: Vec<u8> = {
            let len = fs_info
                .f_fstypename
                .iter()
                .position(|x| *x == 0)
                .unwrap_or(fs_info.f_fstypename.len());
            fs_info.f_fstypename[..len]
                .iter()
                .map(|c| *c as u8)
                .collect()
        };
        match &fs_type[..] {
            b"autofs" | b"devfs" | b"linprocfs" | b"procfs" | b"fdesckfs" | b"tmpfs"
            | b"linsysfs" => {
                sysinfo_debug!(
                    "Memory filesystem `{:?}`, ignoring it.",
                    c_buf_to_utf8_string(&fs_info.f_fstypename).unwrap(),
                );
                continue;
            }
            _ => {}
        }

        if libc::statvfs(fs_info.f_mntonname.as_ptr(), &mut vfs) != 0 {
            continue;
        }

        let mount_point = match c_buf_to_utf8_string(&fs_info.f_mntonname) {
            Some(m) => m,
            None => {
                sysinfo_debug!("Cannot get disk mount point, ignoring it.");
                continue;
            }
        };

        let name = if mount_point == "/" {
            OsString::from("root")
        } else {
            OsString::from(&mount_point)
        };

        // USB keys and CDs are removable.
        let is_removable =
            [b"USB", b"usb"].iter().any(|b| *b == &fs_type[..]) || fs_type.starts_with(b"/dev/cd");

        let f_frsize: u64 = vfs.f_frsize as _;

        container.push(Disk {
            name,
            mount_point: PathBuf::from(&mount_point),
            total_bytes: vfs.f_blocks.saturating_mul(f_frsize),
            available_bytes: vfs.f_favail.saturating_mul(f_frsize),
            file_system: OsString::from_vec(fs_type),
            removable: is_removable,
        });
    }
}

pub(crate) fn disks() -> Vec<Disk> {
    let mut disks = Vec::new();
    unsafe {
        get_all_list(&mut disks);
    }
    disks
}
