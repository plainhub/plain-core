use plain_libs::anyhow::Result;
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
    super::deps::install_all_deps();
    write_service(Path::new(SERVICE_PATH), BINARY_PATH)?;
    if super::packages::has_cmd("systemctl") {
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
        "[Unit]\nDescription=PlainApp service\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nExecStart={binary} run\nRestart=on-failure\nRestartSec=5\nLimitNOFILE=65536\n\n[Install]\nWantedBy=multi-user.target\n"
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
    if !keep_config {
        remove_file(cert)?;
        remove_file(key)?;
    }
    if !keep_data {
        match std::fs::remove_dir_all(data) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    remove_file(Path::new(SERVICE_PATH))
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
