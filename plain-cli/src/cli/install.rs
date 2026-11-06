use anyhow::Result;

#[cfg(target_os = "linux")]
pub fn run() -> Result<()> {
    plain_rs::infrastructure::platform::linux::install::service::install()?;
    println!("plainapp install: done");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn run() -> Result<()> {
    anyhow::bail!(
        "System service installation is available on Linux; use plainapp run on this platform"
    )
}
