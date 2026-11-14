use plain_libs::anyhow::Result;
use plain_rs::features::prefs::{self, settings};
use plain_rs::infrastructure::platform::system::consts::AppPaths;

pub fn run(paths: &AppPaths, enabled: bool) -> Result<()> {
    let _data_directory = plain_rs::infrastructure::data_paths::directory::DataDirectory::open(
        &paths.data_dir,
        &paths.cache_dir,
    )
    .map_err(plain_libs::anyhow::Error::msg)?;
    let prefs = prefs::Prefs::load(&prefs::default_path(&paths.data_dir))?;
    settings::patch(
        &prefs,
        settings::SettingsPatch {
            developer_mode: Some(enabled),
            ..Default::default()
        },
    )?;
    println!(
        "Developer mode {}.",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/plain-cli/unit/cli/dev.rs"]
mod tests;
