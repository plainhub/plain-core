use plain_libs::anyhow::{Result, anyhow};
use plain_libs::platform::linux::disks::blockdev::LsblkDevice;
use plain_libs::platform::linux::disks::blockdev::run_lsblk;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn run_cmd(prog: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| anyhow!("{prog} spawn: {e}"))?;
    if !out.status.success() {
        let combined = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "{prog} failed: {}",
            one_line(&format!("{combined}{stderr}"))
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn one_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ").trim().to_string()
}

pub fn first_partition_path(disk_path: &str) -> String {
    let base = disk_path.trim();
    if base.is_empty() {
        return String::new();
    }
    let last = base.chars().last().unwrap();
    if last.is_ascii_digit() {
        format!("{base}p1")
    } else {
        format!("{base}1")
    }
}

fn find_lsblk_disk<'a>(devs: &'a [LsblkDevice], path: &str) -> Option<&'a LsblkDevice> {
    fn walk<'a>(d: &'a LsblkDevice, path: &str, out: &mut Option<&'a LsblkDevice>) {
        if out.is_some() {
            return;
        }
        let p = d.path.trim();
        let n = d.name.trim();
        let full = std::path::Path::new("/dev").join(n);
        if p == path || (p.is_empty() && !n.is_empty() && full.to_string_lossy() == path) {
            *out = Some(d);
            return;
        }
        for c in &d.children {
            walk(c, path, out);
        }
    }
    let mut found = None;
    for d in devs {
        walk(d, path, &mut found);
        if found.is_some() {
            break;
        }
    }
    found
}

fn has_mountpoint(d: &LsblkDevice, m: &str) -> bool {
    if d.mountpoint.as_deref().map(str::trim) == Some(m) {
        return true;
    }
    d.children.iter().any(|c| has_mountpoint(c, m))
}

fn list_mounted(d: &LsblkDevice) -> Vec<String> {
    fn walk(d: &LsblkDevice, out: &mut Vec<String>) {
        let mp = d.mountpoint.as_deref().unwrap_or("").trim();
        if !mp.is_empty() && !out.iter().any(|s| s == mp) {
            out.push(mp.to_string());
        }
        for c in &d.children {
            walk(c, out);
        }
    }
    let mut v = Vec::new();
    walk(d, &mut v);
    v
}

fn lsblk() -> Result<Vec<LsblkDevice>> {
    run_lsblk(&["NAME", "PATH", "TYPE", "MOUNTPOINT"])
}

fn which(prog: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {prog}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn require_tool(prog: &str) -> Result<()> {
    if which(prog) {
        Ok(())
    } else {
        Err(anyhow!("required tool not found in PATH: {prog}"))
    }
}

fn wait_for_path(path: &str, timeout: Duration) -> Result<()> {
    if path.trim().is_empty() {
        return Err(anyhow!("path is empty"));
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if Path::new(path).exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    Err(anyhow!("timeout waiting for {path}"))
}

fn unmount_volumes(
    mut volumes: Vec<crate::features::audit_events::EventVolume>,
    mut unmount: impl FnMut(&str) -> Result<()>,
    events: &crate::infrastructure::platform::linux::mounts::automount::EventSink,
) -> Result<()> {
    use crate::features::audit_events::{AuditEvent, VolumeEventData};
    use crate::infrastructure::platform::linux::mounts::automount::MountEvent;
    volumes.sort_by(|a, b| {
        b.mount_path
            .len()
            .cmp(&a.mount_path.len())
            .then_with(|| b.mount_path.cmp(&a.mount_path))
    });
    volumes.dedup_by(|a, b| a.mount_path == b.mount_path);
    for volume in volumes {
        unmount(&volume.mount_path)?;
        events(MountEvent::Audit(AuditEvent::Unmount(VolumeEventData {
            volume,
        })))?;
    }
    Ok(())
}

pub fn format_disk_single_partition(
    store: &std::sync::Arc<plain_libs::kv::Db>,
    disk_path: &str,
    events: &crate::infrastructure::platform::linux::mounts::automount::EventSink,
) -> Result<()> {
    let p = disk_path.trim();
    if p.is_empty() {
        return Err(anyhow!("disk path is required"));
    }
    if !p.starts_with("/dev/") {
        return Err(anyhow!("invalid disk path: {p:?}"));
    }

    let release_auto_mount = crate::infrastructure::platform::linux::mounts::automount::inhibit();

    for t in ["wipefs", "sfdisk", "mkfs.ext4", "umount"] {
        require_tool(t)?;
    }
    let has_partprobe = which("partprobe");
    let has_udevadm = which("udevadm");

    let parsed = lsblk()?;
    let disk = find_lsblk_disk(&parsed, p).ok_or_else(|| anyhow!("disk not found: {p}"))?;
    if disk.kind.trim() != "disk" {
        return Err(anyhow!("device is not a disk: {p}"));
    }
    if has_mountpoint(disk, "/") {
        return Err(anyhow!("refusing to format system disk: {p}"));
    }

    let mut mps = list_mounted(disk);
    if !mps.is_empty() {
        let mounts = crate::infrastructure::platform::linux::mounts::list::list_mounts();
        let volumes = mps
            .iter()
            .map(|path| {
                let found = mounts
                    .iter()
                    .find(|mount| mount.mount_point.as_deref() == Some(path));
                crate::features::audit_events::EventVolume {
                    id: found.map(|mount| mount.id.clone()),
                    name: found
                        .map(|mount| mount.label.clone().unwrap_or_else(|| mount.name.clone())),
                    device_path: found.and_then(|mount| mount.path.clone()),
                    mount_path: path.to_owned(),
                    file_system: found.and_then(|mount| mount.fs_type.clone()),
                }
            })
            .collect();
        unmount_volumes(
            volumes,
            |path| run_cmd("umount", &[path]).map(|_| ()),
            events,
        )?;

        let parsed2 = lsblk()?;
        let disk2 = find_lsblk_disk(&parsed2, p).ok_or_else(|| anyhow!("disk not found: {p}"))?;
        if has_mountpoint(disk2, "/") {
            return Err(anyhow!("refusing to format system disk: {p}"));
        }
        mps = list_mounted(disk2);
        if !mps.is_empty() {
            return Err(anyhow!(
                "failed to unmount disk {p} (still mounted: {})",
                mps.join(", ")
            ));
        }
    }

    run_cmd("wipefs", &["-a", p])?;

    let script = "label: gpt\nsize=+, type=linux\n";
    let sfdisk = Command::new("sfdisk")
        .args([
            "--wipe",
            "always",
            "--wipe-partitions",
            "always",
            "--force",
            p,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("sfdisk spawn: {e}"))?;
    {
        let mut stdin = sfdisk
            .stdin
            .as_ref()
            .ok_or_else(|| anyhow!("sfdisk stdin"))?;
        stdin.write_all(script.as_bytes())?;
    }
    let out = sfdisk.wait_with_output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "sfdisk failed: {}",
            one_line(&format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        ));
    }

    if has_partprobe {
        let _ = Command::new("partprobe").arg(p).output();
    }
    if has_udevadm {
        let _ = Command::new("udevadm").arg("settle").output();
    }

    let part_path = first_partition_path(p);
    wait_for_path(&part_path, Duration::from_secs(3))?;

    run_cmd("mkfs.ext4", &["-F", "-L", "plainapp", &part_path])?;

    drop(release_auto_mount);
    crate::infrastructure::platform::linux::mounts::automount::ensure_mounted_usb_volumes(
        store, events,
    )?;

    Ok(())
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/infrastructure/platform/linux/disks/format.rs"]
mod tests;
