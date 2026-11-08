// Take a look at the license at the top of the repository in the LICENSE file.

use crate::system_metrics::Disk;

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use crate::platform::windows::metrics::{
    DRIVE_FIXED, DRIVE_REMOVABLE, ERROR_MORE_DATA, ERROR_NO_MORE_FILES, FindFirstVolumeW,
    FindNextVolumeW, FindVolumeClose, GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
    GetVolumePathNamesForVolumeNameW, Handle, MAX_PATH,
};
use std::{io, ptr::null_mut};

fn from_zero_terminated(buf: &[u16]) -> Option<Vec<u16>> {
    let end = buf.iter().position(|&word| word == 0)?;
    Some(buf[..=end].to_vec())
}

struct VolumeSearch(Handle);
impl Drop for VolumeSearch {
    fn drop(&mut self) {
        if unsafe { FindVolumeClose(self.0) } == 0 {
            sysinfo_debug!(
                "Cannot close volume enumeration: {}",
                io::Error::last_os_error()
            );
        }
    }
}

fn volume_paths(mut buffer: &[u16]) -> Option<Vec<Vec<u16>>> {
    let mut paths = Vec::new();
    loop {
        if *buffer.first()? == 0 {
            return Some(paths);
        }
        let path = from_zero_terminated(buffer)?;
        buffer = &buffer[path.len()..];
        paths.push(path);
    }
}

// Realistically, volume names are probably not longer than 44 characters,
// but the example in the Microsoft documentation uses MAX_PATH as well.
// https://learn.microsoft.com/en-us/windows/win32/fileio/displaying-volume-paths
const VOLUME_NAME_SIZE: usize = MAX_PATH + 1;

pub(crate) fn get_volume_guid_paths() -> Vec<Vec<u16>> {
    let mut volume_names = Vec::new();
    unsafe {
        let mut buf = Box::new([0u16; VOLUME_NAME_SIZE]);
        let handle = FindFirstVolumeW(buf.as_mut_ptr(), buf.len() as u32);
        if handle.is_null() || handle.addr() == usize::MAX {
            sysinfo_debug!("Error: FindFirstVolumeW() = {}", io::Error::last_os_error());
            return Vec::new();
        }
        let search = VolumeSearch(handle);
        let Some(name) = from_zero_terminated(&buf[..]) else {
            sysinfo_debug!("Volume enumeration returned an unterminated name");
            return Vec::new();
        };
        volume_names.push(name);
        loop {
            if FindNextVolumeW(search.0, buf.as_mut_ptr(), buf.len() as u32) == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_NO_MORE_FILES) {
                    sysinfo_debug!("Error: FindNextVolumeW = {error}");
                }
                break;
            }
            let Some(name) = from_zero_terminated(&buf[..]) else {
                sysinfo_debug!("Volume enumeration returned an unterminated name");
                return Vec::new();
            };
            volume_names.push(name);
        }
    }
    volume_names
}

pub(crate) unsafe fn get_volume_path_names_for_volume_name(
    volume_guid_path: &[u16],
) -> Vec<Vec<u16>> {
    let mut path_names_buf = vec![0u16; MAX_PATH];
    let mut path_names_output_size = 0u32;
    loop {
        let result = unsafe {
            GetVolumePathNamesForVolumeNameW(
                volume_guid_path.as_ptr(),
                path_names_buf.as_mut_ptr(),
                path_names_buf.len() as u32,
                &mut path_names_output_size,
            )
        };
        if result != 0 {
            let Some(buffer) = path_names_buf.get(..path_names_output_size as usize) else {
                sysinfo_debug!("Volume paths exceed the supplied buffer");
                return Vec::new();
            };
            return volume_paths(buffer).unwrap_or_else(|| {
                sysinfo_debug!("Volume paths are not terminated");
                Vec::new()
            });
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_MORE_DATA)
            || path_names_output_size as usize <= path_names_buf.len()
        {
            sysinfo_debug!("Cannot read volume paths: {error}");
            return Vec::new();
        }
        path_names_buf.resize(path_names_output_size as usize, 0);
    }
}

unsafe fn get_drive_size(mount_point: &[u16]) -> Option<(u64, u64)> {
    let mut total_size = 0;
    let mut available_space = 0;
    let result = unsafe {
        GetDiskFreeSpaceExW(
            mount_point.as_ptr(),
            null_mut(),
            &mut total_size,
            &mut available_space,
        )
    };
    (result != 0).then_some((total_size, available_space))
}

pub(crate) fn get_list() -> Vec<Disk> {
    get_volume_guid_paths()
        .into_iter()
        .flat_map(|volume_name| {
            let raw_volume_name = volume_name.as_ptr();
            let drive_type = unsafe { GetDriveTypeW(raw_volume_name) };

            let is_removable = drive_type == DRIVE_REMOVABLE;

            if drive_type != DRIVE_FIXED && drive_type != DRIVE_REMOVABLE {
                return Vec::new();
            }
            let mut name = [0u16; MAX_PATH + 1];
            let mut file_system = [0u16; 32];
            let mut flags = 0;
            let volume_info = unsafe {
                GetVolumeInformationW(
                    raw_volume_name,
                    name.as_mut_ptr(),
                    name.len() as u32,
                    null_mut(),
                    null_mut(),
                    &mut flags,
                    file_system.as_mut_ptr(),
                    file_system.len() as u32,
                )
            };
            if volume_info == 0 {
                sysinfo_debug!(
                    "Error: GetVolumeInformationW = {}",
                    io::Error::last_os_error()
                );
                return Vec::new();
            }

            let mount_paths = unsafe { get_volume_path_names_for_volume_name(&volume_name) };
            if mount_paths.is_empty() {
                return Vec::new();
            }

            let Some((total_space, available_space)) = (unsafe { get_drive_size(&mount_paths[0]) })
            else {
                return Vec::new();
            };
            if total_space == 0 {
                sysinfo_debug!("total_space == 0");
                return Vec::new();
            }
            let name = os_string_from_zero_terminated(&name);
            let file_system = os_string_from_zero_terminated(&file_system);
            mount_paths
                .into_iter()
                .map(move |mount_path| Disk {
                    name: name.clone(),
                    file_system: file_system.clone(),
                    mount_point: OsString::from_wide(&mount_path[..mount_path.len() - 1]).into(),
                    total_bytes: total_space,
                    available_bytes: available_space,
                    removable: is_removable,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

fn os_string_from_zero_terminated(name: &[u16]) -> OsString {
    let len = name.iter().position(|&x| x == 0).unwrap_or(name.len());
    OsString::from_wide(&name[..len])
}

pub(crate) fn disks() -> Vec<Disk> {
    get_list()
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/system_metrics/windows/disk.rs"]
mod tests;
