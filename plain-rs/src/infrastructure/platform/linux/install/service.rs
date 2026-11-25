use crate::infrastructure::data_paths::DataPaths;
use plain_libs::anyhow::{Result, ensure};
use std::fs;
use std::path::Path;

pub const SERVICE_PATH: &str = "/etc/systemd/system/plainapp.service";
pub const BINARY_PATH: &str = "/usr/local/bin/plainapp";

pub fn require_root() -> Result<()> {
    plain_libs::anyhow::ensure!(
        plain_libs::platform::unix::is_root(),
        "This command requires root privileges. Please run with sudo."
    );
    Ok(())
}

pub fn install() -> Result<()> {
    require_root()?;
    install_unit(Path::new(SERVICE_PATH), BINARY_PATH)
}

fn install_unit(path: &Path, binary: &str) -> Result<()> {
    write_service(path, binary)?;
    if std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| directory.join("systemctl").is_file())
    }) {
        let status = std::process::Command::new("systemctl")
            .arg("daemon-reload")
            .status()?;
        plain_libs::anyhow::ensure!(status.success(), "systemd unit reload failed");
    }
    Ok(())
}

pub fn write_service(path: &Path, binary: &str) -> Result<()> {
    let unit = render_service(binary);
    if path.exists() && std::fs::read_to_string(path)? == unit {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, unit)?;
    Ok(())
}

fn render_service(binary: &str) -> String {
    format!(
        "[Unit]\nDescription=PlainApp service\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nUser=root\nExecStart={binary} run\nRestart=on-failure\nRestartSec=5\nLimitNOFILE=65536\n\n[Install]\nWantedBy=multi-user.target\n"
    )
}

fn remove_file(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn uninstall(
    data: &Path,
    cert: &Path,
    key: &Path,
    keep_config: bool,
    keep_data: bool,
) -> Result<()> {
    require_root()?;
    let unit = Path::new(SERVICE_PATH);
    let binary = Path::new(BINARY_PATH);
    let directory = fs::File::open(binary.parent().unwrap())?;
    directory.try_lock()?;
    let paths = DataPaths::new(data);
    if !keep_data && data.try_exists()? {
        ensure!(
            data.parent().is_some()
                && fs::symlink_metadata(data)?.is_dir()
                && (paths.lock().is_file() || paths.database().is_file()),
            "Refusing to remove an unrecognized PlainApp data directory"
        );
    }
    if unit.try_exists()? {
        ensure!(
            fs::symlink_metadata(unit)?.is_file()
                && fs::read_to_string(unit)?
                    .lines()
                    .any(|line| line == "ExecStart=/usr/local/bin/plainapp run"),
            "Refusing to uninstall an unmanaged PlainApp service"
        );
        let status = std::process::Command::new("systemctl")
            .args(["disable", "--now", "plainapp.service"])
            .status()?;
        ensure!(
            status.success(),
            "Could not stop and disable the PlainApp service"
        );
        remove_file(unit)?;
        let status = std::process::Command::new("systemctl")
            .arg("daemon-reload")
            .status()?;
        ensure!(status.success(), "systemd unit reload failed");
    }
    if !keep_config {
        remove_file(cert)?;
        remove_file(key)?;
    }
    if !keep_data {
        if keep_config {
            remove_data_except(
                data,
                &[
                    &paths.prefs(),
                    &paths.tls_identity(),
                    cert,
                    key,
                    &paths.lock(),
                ],
            )?;
        } else if data.try_exists()? {
            fs::remove_dir_all(data)?;
        }
    }
    remove_file(binary)
}

fn remove_data_except(directory: &Path, preserve: &[&Path]) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if preserve.contains(&path.as_path()) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            if preserve.iter().any(|file| file.starts_with(&path)) {
                remove_data_except(&path, preserve)?;
            } else {
                fs::remove_dir_all(&path)?;
            }
        } else {
            remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn restart() -> Result<()> {
    restart_service(&mut std::process::Command::new("systemd-run"))
}

fn restart_service(command: &mut std::process::Command) -> Result<()> {
    let status = command
        .args([
            "--scope",
            "--quiet",
            "--collect",
            "--no-ask-password",
            "--",
            "systemctl",
            "--no-block",
            "restart",
            "plainapp.service",
        ])
        .status()?;
    plain_libs::anyhow::ensure!(status.success(), "service restart failed");
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/infrastructure/platform/linux/install/service.rs"]
mod tests;
