use anyhow::Result;
use plain_rs::infrastructure::platform::system::consts::AppPaths;

#[cfg(target_os = "linux")]
pub fn run(paths: &AppPaths) -> Result<()> {
    plain_rs::infrastructure::platform::linux::install::service::require_root()?;
    anyhow::ensure!(
        std::env::var_os("PLAINAPP_DATA_DIR").is_none()
            && std::env::var_os("PLAINAPP_CACHE_DIR").is_none(),
        "System service installation requires default directories; unset PLAINAPP_DATA_DIR and PLAINAPP_CACHE_DIR before installing"
    );
    prepare_password(paths, super::passwd::prompt_password)?;
    plain_rs::infrastructure::platform::linux::install::service::install()?;
    println!("plainapp install: done");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn run(_paths: &AppPaths) -> Result<()> {
    anyhow::bail!(
        "System service installation is available on Linux; use plainapp passwd and plainapp run on this platform"
    )
}

#[cfg(any(target_os = "linux", test))]
fn prepare_password(paths: &AppPaths, prompt: impl FnOnce() -> Result<String>) -> Result<()> {
    use plain_rs::features::prefs::{Prefs, default_path, security};
    let prefs_path = default_path(&paths.data_dir);
    let prefs = Prefs::load(&prefs_path)?;
    if matches!(security::password_hash(&prefs), Ok(Some(_))) {
        return Ok(());
    }
    let _directory = plain_rs::infrastructure::data_paths::directory::DataDirectory::open(
        &paths.data_dir,
        &paths.cache_dir,
    )
    .map_err(anyhow::Error::msg)?;
    let prefs = Prefs::load(&prefs_path)?;
    security::bootstrap(&prefs)?;
    if security::password_hash(&prefs)?.is_none() {
        security::set_password_login(&prefs, &prompt()?)?;
        println!("Admin password configured.");
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/plain-cli/unit/cli/install.rs"]
mod tests;
