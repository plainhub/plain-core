// Take a look at the license at the top of the repository in the LICENSE file.

use crate::system_metrics::Disk;
use crate::system_metrics::sys::utils::{get_all_utf8_data, to_cpath};

use libc::statvfs;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};

macro_rules! cast {
    ($x:expr) => {
        u64::from($x)
    };
}

fn new_disk(
    device_name: &OsStr,
    mount_point: &Path,
    file_system: &OsStr,
    removable_entries: &[PathBuf],
) -> Option<Disk> {
    let mount_point_cpath = to_cpath(mount_point);
    let mut total = 0;
    let mut available = 0;
    unsafe {
        let mut stat: statvfs = mem::zeroed();
        if retry_eintr!(statvfs(mount_point_cpath.as_ptr() as *const _, &mut stat)) == 0 {
            let bsize = cast!(stat.f_bsize);
            let blocks = cast!(stat.f_blocks);
            let bavail = cast!(stat.f_bavail);
            total = bsize.saturating_mul(blocks);
            available = bsize.saturating_mul(bavail);
        }
        if total == 0 {
            return None;
        }
        let mount_point = mount_point.to_owned();
        let is_removable = removable_entries
            .iter()
            .any(|e| e.as_os_str() == device_name);
        Some(Disk {
            name: device_name.to_owned(),
            file_system: file_system.to_owned(),
            mount_point,
            total_bytes: cast!(total),
            available_bytes: cast!(available),
            removable: is_removable,
        })
    }
}

fn get_all_list(container: &mut Vec<Disk>, content: &str) {
    container.clear();
    // The goal of this array is to list all removable devices (the ones whose name starts with
    // "usb-").
    let removable_entries = match fs::read_dir("/dev/disk/by-id/") {
        Ok(r) => r
            .filter_map(|res| Some(res.ok()?.path()))
            .filter_map(|e| {
                if e.file_name()
                    .and_then(|x| Some(x.to_str()?.starts_with("usb-")))
                    .unwrap_or_default()
                {
                    e.canonicalize().ok()
                } else {
                    None
                }
            })
            .collect::<Vec<PathBuf>>(),
        _ => Vec::new(),
    };

    for disk in content
        .lines()
        .map(|line| {
            let line = line.trim_start();
            // mounts format
            // http://man7.org/linux/man-pages/man5/fstab.5.html
            // fs_spec<tab>fs_file<tab>fs_vfstype<tab>other fields
            let mut fields = line.split_whitespace();
            let fs_spec = fields.next().unwrap_or("");
            let fs_file = fields
                .next()
                .unwrap_or("")
                .replace("\\134", "\\")
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\012", "\n");
            let fs_vfstype = fields.next().unwrap_or("");
            (fs_spec, fs_file, fs_vfstype)
        })
        .filter(|(fs_spec, fs_file, fs_vfstype)| {
            // Check if fs_vfstype is one of our 'ignored' file systems.
            let filtered = match *fs_vfstype {
                "rootfs" | // https://www.kernel.org/doc/Documentation/filesystems/ramfs-rootfs-initramfs.txt
                "sysfs" | // pseudo file system for kernel objects
                "proc" |  // another pseudo file system
                "devtmpfs" |
                "cgroup" |
                "cgroup2" |
                "pstore" | // https://www.kernel.org/doc/Documentation/ABI/testing/pstore
                "squashfs" | // squashfs is a compressed read-only file system (for snaps)
                "rpc_pipefs" | // The pipefs pseudo file system service
                "iso9660" // optical media
                => true,
                "tmpfs" => !cfg!(any()),
                // calling statvfs on a mounted CIFS or NFS may hang, when they are mounted with option: hard
                "cifs" | "nfs" | "nfs4" => !cfg!(any()),
                _ => false,
            };

            !(filtered ||
               fs_file.starts_with("/sys") || // check if fs_file is an 'ignored' mount point
               fs_file.starts_with("/proc") ||
               (fs_file.starts_with("/run") && !fs_file.starts_with("/run/media")) ||
               fs_spec.starts_with("sunrpc"))
        })
        .filter_map(|(fs_spec, fs_file, fs_vfstype)| {
            new_disk(
                fs_spec.as_ref(),
                Path::new(&fs_file),
                fs_vfstype.as_ref(),
                &removable_entries,
            )
        })
    {
        container.push(disk);
    }
}

// #[test]

pub(crate) fn disks() -> Vec<Disk> {
    let mut disks = Vec::new();
    get_all_list(
        &mut disks,
        &get_all_utf8_data("/proc/mounts", 16_385).unwrap_or_default(),
    );
    disks
}
