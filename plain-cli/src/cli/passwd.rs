use anyhow::Result;
use plain_rs::platform::system::consts::AppPaths;

pub fn run(paths: &AppPaths, stdin: bool) -> Result<()> {
    let password = if stdin {
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        password.trim_end_matches(['\r', '\n']).to_owned()
    } else {
        prompt_password()?
    };
    anyhow::ensure!(!password.is_empty(), "Password cannot be empty");
    set_password(&paths.data_dir, &paths.cache_dir, &password)?;
    println!("Admin password updated.");
    Ok(())
}

#[cfg(unix)]
fn prompt_password() -> Result<String> {
    let password = plain_rs::platform::unix::read_password::prompt_password("New password: ")?;
    let confirm = plain_rs::platform::unix::read_password::prompt_password("Confirm: ")?;
    anyhow::ensure!(password == confirm, "Passwords do not match");
    Ok(password)
}

#[cfg(not(unix))]
fn prompt_password() -> Result<String> {
    anyhow::bail!("Use passwd --stdin to read a password from standard input on this platform")
}

fn set_password(
    directory: &std::path::Path,
    cache: &std::path::Path,
    password: &str,
) -> Result<()> {
    let _data_directory = plain_rs::data_paths::directory::DataDirectory::open(directory, cache)
        .map_err(anyhow::Error::msg)?;
    let prefs = plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(directory))?;
    plain_rs::prefs::security::Command::SetPassword {
        password: password.to_owned(),
    }
    .apply(&prefs)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/cli/passwd.rs"]
mod tests;
