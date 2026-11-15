use plain_libs::anyhow::{Result, anyhow};
use plain_rs::infrastructure::platform::system::consts::AppPaths;

pub async fn run(paths: &AppPaths, key: &str, value: &str) -> Result<()> {
    let value = plain_libs::serde_json::from_str(value)
        .map_err(|_| anyhow!("Preference value must be valid JSON"))?;
    crate::management_socket::set_pref(&paths.data_dir, key.to_owned(), value)
        .await
        .map_err(|error| {
            if crate::management_socket::not_running(&error) {
                anyhow!("PlainApp is not running for the selected data directory")
            } else {
                error
            }
        })?;
    println!("Preference updated: {key}");
    if matches!(key, "http_port" | "https_port") {
        super::status::run(paths).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/plain-cli/unit/cli/prefs.rs"]
mod tests;
