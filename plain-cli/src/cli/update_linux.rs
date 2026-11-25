use crate::management_socket::{self, ListenerState};
use plain_libs::anyhow::{Context, Result, bail, ensure};
use plain_libs::crypto::hash::sha256_reader;
use plain_rs::infrastructure::platform::linux::install::service;
use plain_rs::infrastructure::platform::system::consts::AppPaths;
use serde::Deserialize;
use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const REPOSITORY: &str = "plainhub/plain-core";
const UNIT: &str = "plainapp.service";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

pub(super) fn run(paths: &AppPaths) -> Result<()> {
    service::require_root()?;
    let target = Path::new(service::BINARY_PATH);
    let parent = target.parent().unwrap();
    let lock = File::open(parent)?;
    lock.try_lock().context("Another update is in progress")?;
    let installed_file = fs::symlink_metadata(target)?;
    ensure!(
        installed_file.is_file() && installed_file.uid() == 0 && installed_file.mode() & 0o022 == 0,
        "Expected a root-owned PlainApp executable"
    );
    let installed = String::from_utf8(checked(Command::new(target).arg("--version"))?.stdout)?;
    let installed = installed
        .trim()
        .strip_prefix("plainapp ")
        .context("Installed executable did not report a PlainApp version")?;

    let staging = plain_libs::temporary::Builder::new()
        .prefix(".plainapp-update-")
        .tempdir_in(parent)?;
    let latest = staging.path().join("latest.json");
    download(
        &format!("https://api.github.com/repos/{REPOSITORY}/releases/latest"),
        &latest,
        1024 * 1024,
    )?;
    let release: Release = plain_libs::serde_json::from_slice(&fs::read(latest)?)?;
    ensure!(
        !release.draft && !release.prerelease,
        "Expected a published stable release"
    );
    let next = release
        .tag_name
        .strip_prefix('v')
        .context("Invalid release tag")?;
    match super::version(next)?.cmp(&super::version(installed)?) {
        std::cmp::Ordering::Equal => {
            println!("PlainApp {installed} is up to date.");
            return Ok(());
        }
        std::cmp::Ordering::Less => bail!("Latest release is older than the installed version"),
        std::cmp::Ordering::Greater => {}
    }

    let public_key = include_str!("../../../release-signing.pub").trim_end();
    let archive = match std::env::consts::ARCH {
        "x86_64" => "plainapp-linux-amd64.tar.gz",
        "aarch64" => "plainapp-linux-arm64.tar.gz",
        _ => bail!("Unsupported update architecture"),
    };
    let base = format!(
        "https://github.com/{REPOSITORY}/releases/download/{}",
        release.tag_name
    );
    let manifest = staging.path().join("release-manifest.txt");
    let signature = staging.path().join("release-manifest.sig");
    download(&format!("{base}/release-manifest.txt"), &manifest, 4096)?;
    download(&format!("{base}/release-manifest.sig"), &signature, 64)?;
    let manifest = fs::read(manifest)?;
    let expected = super::archive_hash(
        &manifest,
        &fs::read(signature)?,
        public_key,
        next,
        &sha256_reader(include_bytes!("../../../schema/database.sql").as_slice())?,
        archive,
    )?;

    let package = staging.path().join(archive);
    download(&format!("{base}/{archive}"), &package, 128 * 1024 * 1024)?;
    ensure!(
        sha256_reader(File::open(&package)?)? == expected,
        "Release archive checksum mismatch"
    );
    let files = checked(Command::new("tar").arg("-tzf").arg(&package))?;
    ensure!(
        files.stdout == b"plainapp\n",
        "Unexpected release archive contents"
    );
    let staged = staging.path().join("plainapp");
    checked(
        Command::new("tar")
            .arg("-xOzf")
            .arg(&package)
            .arg("plainapp")
            .stdout(Stdio::from(File::create(&staged)?)),
    )?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
    File::open(&staged)?.sync_all()?;
    let actual = checked(Command::new(&staged).arg("--version"))?;
    ensure!(
        String::from_utf8(actual.stdout)?.trim() == format!("plainapp {next}"),
        "Release binary reported an unexpected version"
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let old_pid = main_pid()?;
    verify_service(old_pid, target)?;
    let old_status = runtime.block_on(management_status(paths))?;
    ensure!(
        old_status.pid == old_pid && old_status.listeners == ListenerState::Running,
        "PlainApp system service is not ready for an update"
    );
    let previous = staging.path().join("plainapp.previous");
    fs::hard_link(target, &previous)?;
    fs::rename(&staged, target)?;
    let result = lock
        .sync_all()
        .map_err(Into::into)
        .and_then(|()| restart_and_check(&runtime, paths, old_pid, target));
    if let Err(error) = result {
        if main_pid().is_ok_and(|pid| pid == old_pid)
            && verify_service(old_pid, &previous).is_ok()
            && fs::rename(&previous, target).is_ok()
        {
            lock.sync_all()?;
            bail!(
                "Update could not be confirmed: {error:#}. Restored the executable still running in the old service."
            );
        }
        let backup = staging.keep().join("plainapp.previous");
        bail!(
            "Update could not be confirmed: {error:#}. Previous executable saved at {}. Review service logs and data compatibility before restoring it.",
            backup.display()
        );
    }
    println!("Updated PlainApp {installed} to {next}.");
    Ok(())
}

fn download(url: &str, destination: &Path, limit: u64) -> Result<()> {
    checked(
        Command::new("curl")
            .args([
                "--disable",
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "15",
                "--max-time",
                "600",
                "--max-filesize",
            ])
            .arg(limit.to_string())
            .arg("--output")
            .arg(destination)
            .arg("--url")
            .arg(url),
    )?;
    ensure!(
        fs::metadata(destination)?.len() <= limit,
        "Release download exceeded its size limit"
    );
    Ok(())
}

fn checked(command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("Run {:?}", command.get_program()))?;
    ensure!(
        output.status.success(),
        "{:?} failed: {}",
        command.get_program(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output)
}

fn main_pid() -> Result<u32> {
    let output =
        checked(Command::new("systemctl").args(["show", "--property=MainPID", "--value", UNIT]))?;
    let pid = std::str::from_utf8(&output.stdout)?.trim().parse()?;
    ensure!(pid != 0, "PlainApp system service is not running");
    Ok(pid)
}

fn verify_service(pid: u32, executable: &Path) -> Result<()> {
    let running = fs::metadata(PathBuf::from(format!("/proc/{pid}/exe")))?;
    let installed = fs::symlink_metadata(executable)?;
    ensure!(
        installed.is_file() && (running.dev(), running.ino()) == (installed.dev(), installed.ino()),
        "PlainApp service is not running the managed executable"
    );
    Ok(())
}

async fn management_status(paths: &AppPaths) -> Result<management_socket::Status> {
    tokio::time::timeout(
        Duration::from_secs(2),
        management_socket::status(&paths.data_dir),
    )
    .await?
}

fn restart_and_check(
    runtime: &tokio::runtime::Runtime,
    paths: &AppPaths,
    old_pid: u32,
    target: &Path,
) -> Result<()> {
    checked(Command::new("systemctl").args(["--no-block", "restart", UNIT]))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(status) = runtime.block_on(management_status(paths))
            && status.pid != old_pid
            && status.listeners == ListenerState::Running
            && main_pid().is_ok_and(|pid| pid == status.pid)
            && verify_service(status.pid, target).is_ok()
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!("PlainApp did not report healthy listeners after restart")
}
