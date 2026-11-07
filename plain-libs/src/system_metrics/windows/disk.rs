// Take a look at the license at the top of the repository in the LICENSE file.

use crate::system_metrics::Disk;

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, MAX_PATH};
use windows::Win32::Storage::FileSystem::{
    FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetDiskFreeSpaceExW, GetDriveTypeW,
    GetVolumeInformationW, GetVolumePathNamesForVolumeNameW,
};
use windows::Win32::System::WindowsProgramming::{DRIVE_FIXED, DRIVE_REMOVABLE};
use windows::core::{Error, HRESULT, PCWSTR};

fn from_zero_terminated(buf: &[u16]) -> Option<Vec<u16>> {
    let end = buf.iter().position(|&word| word == 0)?;
    Some(buf[..=end].to_vec())
}

struct VolumeSearch(HANDLE);
impl Drop for VolumeSearch {
    fn drop(&mut self) {
        if let Err(error) = unsafe { FindVolumeClose(self.0) } {
            sysinfo_debug!("Cannot close volume enumeration: {error}");
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
const VOLUME_NAME_SIZE: usize = MAX_PATH as usize + 1;

const ERROR_NO_MORE_FILES: HRESULT = windows::Win32::Foundation::ERROR_NO_MORE_FILES.to_hresult();
const ERROR_MORE_DATA: HRESULT = windows::Win32::Foundation::ERROR_MORE_DATA.to_hresult();

pub(crate) fn get_volume_guid_paths() -> Vec<Vec<u16>> {
    let mut volume_names = Vec::new();
    unsafe {
        let mut buf = Box::new([0u16; VOLUME_NAME_SIZE]);
        let Ok(handle) = FindFirstVolumeW(&mut buf[..]) else {
            sysinfo_debug!(
                "Error: FindFirstVolumeW() = {:?}",
                Error::from_win32().code()
            );
            return Vec::new();
        };
        let search = VolumeSearch(handle);
        let Some(name) = from_zero_terminated(&buf[..]) else {
            sysinfo_debug!("Volume enumeration returned an unterminated name");
            return Vec::new();
        };
        volume_names.push(name);
        loop {
            if FindNextVolumeW(search.0, &mut buf[..]).is_err() {
                if Error::from_win32().code() != ERROR_NO_MORE_FILES {
                    sysinfo_debug!("Error: FindNextVolumeW = {}", Error::from_win32().code());
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
    let volume_guid_path = PCWSTR::from_raw(volume_guid_path.as_ptr());

    // Initial buffer size is just a guess. There is no direct connection between MAX_PATH
    // the output of GetVolumePathNamesForVolumeNameW.
    let mut path_names_buf = vec![0u16; MAX_PATH as usize];
    let mut path_names_output_size = 0u32;
    loop {
        match GetVolumePathNamesForVolumeNameW(
            volume_guid_path,
            Some(path_names_buf.as_mut_slice()),
            &mut path_names_output_size,
        ) {
            Ok(()) => {
                let Some(buffer) = path_names_buf.get(..path_names_output_size as usize) else {
                    sysinfo_debug!("Volume paths exceed the supplied buffer");
                    return Vec::new();
                };
                return volume_paths(buffer).unwrap_or_else(|| {
                    sysinfo_debug!("Volume paths are not terminated");
                    Vec::new()
                });
            }
            Err(error)
                if error.code() == ERROR_MORE_DATA
                    && path_names_output_size as usize > path_names_buf.len() =>
            {
                path_names_buf.resize(path_names_output_size as usize, 0);
            }
            Err(error) => {
                sysinfo_debug!("Cannot read volume paths: {error}");
                return Vec::new();
            }
        }
    }
}

unsafe fn get_drive_size(mount_point: &[u16]) -> Option<(u64, u64)> {
    let mut total_size = 0;
    let mut available_space = 0;
    let lpdirectoryname = PCWSTR::from_raw(mount_point.as_ptr());
    if GetDiskFreeSpaceExW(
        lpdirectoryname,
        None,
        Some(&mut total_size),
        Some(&mut available_space),
    )
    .is_ok()
    {
        Some((total_size, available_space))
    } else {
        None
    }
}

pub(crate) unsafe fn get_list() -> Vec<Disk> {
    get_volume_guid_paths()
        .into_iter()
        .flat_map(|volume_name| {
            let raw_volume_name = PCWSTR::from_raw(volume_name.as_ptr());
            let drive_type = GetDriveTypeW(raw_volume_name);

            let is_removable = drive_type == DRIVE_REMOVABLE;

            if drive_type != DRIVE_FIXED && drive_type != DRIVE_REMOVABLE {
                return Vec::new();
            }
            let mut name = [0u16; MAX_PATH as usize + 1];
            let mut file_system = [0u16; 32];
            let mut flags = 0;
            let volume_info_res = GetVolumeInformationW(
                raw_volume_name,
                Some(&mut name),
                None,
                None,
                Some(&mut flags),
                Some(&mut file_system),
            )
            .is_ok();
            if !volume_info_res {
                sysinfo_debug!(
                    "Error: GetVolumeInformationW = {:?}",
                    Error::from_win32().code()
                );
                return Vec::new();
            }

            let mount_paths = get_volume_path_names_for_volume_name(&volume_name[..]);
            if mount_paths.is_empty() {
                return Vec::new();
            }

            let Some((total_space, available_space)) = get_drive_size(&mount_paths[0][..]) else {
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
    unsafe { get_list() }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/system_metrics/windows/disk.rs"]
mod tests;
