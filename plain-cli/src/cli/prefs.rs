use plain_libs::anyhow::Result;
use plain_rs::features::prefs::{self, Prefs};
use plain_rs::features::services::settings::preferences::Preferences;
use plain_rs::infrastructure::platform::system::consts::AppPaths;

pub async fn run(paths: &AppPaths, key: &str, value: &str) -> Result<()> {
    let value = plain_libs::serde_json::from_str(value)?;
    let _data_directory = plain_rs::infrastructure::data_paths::directory::DataDirectory::open(
        &paths.data_dir,
        &paths.cache_dir,
    )
    .map_err(plain_libs::anyhow::Error::msg)?;
    let prefs = Prefs::load(&prefs::default_path(&paths.data_dir))?;
    Preferences {
        prefs: &prefs,
        events: None,
        server: None,
    }
    .set(key, value)
    .await?;
    println!("Preference updated: {key}");
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/plain-cli/unit/cli/prefs.rs"]
mod tests;
