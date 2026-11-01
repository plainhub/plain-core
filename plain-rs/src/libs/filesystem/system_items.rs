#[cfg(any(target_os = "linux", target_os = "android", test))]
const UNIX_DIRECTORIES: &[&str] = &[
    "/bin",
    "/sbin",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/boot",
    "/dev",
    "/etc",
    "/proc",
    "/sys",
    "/run",
    "/tmp",
    "/usr",
    "/var",
    "/opt",
    "/srv",
    "/root",
    "/lost+found",
    "/snap",
    "/bin.usr-is-merged",
    "/sbin.usr-is-merged",
    "/lib.usr-is-merged",
];

pub fn contains(path: &str) -> Result<bool, String> {
    #[cfg(target_os = "linux")]
    {
        if UNIX_DIRECTORIES.contains(&path) {
            return Ok(true);
        }
        if std::path::Path::new(path).parent() != Some(std::path::Path::new("/")) {
            return Ok(false);
        }
        match std::fs::read_to_string("/proc/swaps") {
            Ok(contents) => Ok(swap_files(&contents).iter().any(|item| item == path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }
    #[cfg(target_os = "android")]
    {
        Ok(android(path))
    }
    #[cfg(windows)]
    {
        use crate::libs::platform::known_folders::{Folder, resolve};
        static DIRECTORIES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let directories = DIRECTORIES.get_or_init(|| {
            [
                Folder::Windows,
                Folder::ProgramFiles,
                Folder::ProgramFilesX86,
                Folder::ProgramFilesX64,
                Folder::ProgramData,
            ]
            .into_iter()
            .filter_map(resolve)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect()
        });
        Ok(windows(path, directories))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        let _ = path;
        Ok(false)
    }
}

#[cfg(any(target_os = "linux", test))]
fn swap_files(contents: &str) -> Vec<String> {
    contents
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let path = fields.next()?;
            (fields.next()? == "file").then(|| {
                path.replace("\\040", " ")
                    .replace("\\011", "\t")
                    .replace("\\012", "\n")
                    .replace("\\134", "\\")
            })
        })
        .collect()
}

#[cfg(any(target_os = "android", test))]
fn android(path: &str) -> bool {
    if UNIX_DIRECTORIES.contains(&path)
        || matches!(
            path,
            "/system"
                | "/system_ext"
                | "/vendor"
                | "/product"
                | "/odm"
                | "/oem"
                | "/apex"
                | "/data"
                | "/metadata"
                | "/cache"
                | "/config"
                | "/acct"
                | "/linkerconfig"
                | "/debug_ramdisk"
                | "/second_stage_resources"
        )
    {
        return true;
    }
    if matches!(path, "/sdcard/Android" | "/mnt/sdcard/Android") {
        return true;
    }
    let Some(volume) = path
        .strip_prefix("/storage/")
        .and_then(|p| p.strip_suffix("/Android"))
    else {
        return false;
    };
    if let Some(user) = volume.strip_prefix("emulated/") {
        return !user.is_empty() && user.bytes().all(|byte| byte.is_ascii_digit());
    }
    !volume.is_empty() && !volume.contains('/')
}

#[cfg(any(windows, test))]
fn windows(path: &str, directories: &[String]) -> bool {
    let path = path.replace('\\', "/");
    directories
        .iter()
        .any(|item| item.eq_ignore_ascii_case(&path))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/filesystem/system_items.rs"]
mod tests;
