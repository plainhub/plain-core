use plain_rs::runtime::Runtime as CoreRuntime;
use std::{fmt::Write as _, io::Write as _};

pub async fn write(state: &CoreRuntime, report: String) -> Result<String, String> {
    let info = plain_rs::services::device::info::collect(
        &state.host,
        state.device.directory().to_path_buf(),
        [
            "name",
            "osName",
            "osVersion",
            "appVersion",
            "appBuildNumber",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        state.device.fixed_info()?,
    )
    .await?;
    let channel = &state.host.startup().build_channel;
    let mut body = String::new();
    let version = if info.app_build_number.is_empty() {
        info.app_version.clone()
    } else {
        format!("{} ({})", info.app_version, info.app_build_number)
    };
    write!(body, "App version: {version}\nOS: {} {}\nDevice: {}\nPlatform: {}\nBuild type: {}\n\n--- Crash Report ---\n{report}",
        info.os_name, info.os_version, info.name, format!("{:?}", info.platform).to_lowercase(), channel.to_lowercase())
        .map_err(|error| error.to_string())?;
    match state.logs.read(String::new(), 0, 200).await {
        Ok(mut lines) if !lines.is_empty() => {
            lines.reverse();
            body.push_str("\n\n--- App Logs ---\n");
            body.push_str(&lines.join("\n"));
        }
        Ok(_) => {}
        Err(error) => log::warn!("Unable to include app logs in crash report: {error}"),
    }
    let path = state.cache.crash_report_export();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(path.parent().ok_or("Invalid report directory")?)
            .map_err(|error| error.to_string())?;
        let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())
            .map_err(|error| error.to_string())?;
        temporary
            .write_all(body.as_bytes())
            .map_err(|error| error.to_string())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        temporary
            .persist(&path)
            .map_err(|error| error.to_string())?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}
