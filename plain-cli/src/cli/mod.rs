mod help;
mod options;
mod parser;

pub mod bench;
pub mod install;
pub mod login;
pub mod passwd;
pub mod run;
pub mod uninstall;
pub mod update;

use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct Cli {
    pub(crate) data_dir: Option<PathBuf>,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) tls_cert: Option<PathBuf>,
    pub(crate) tls_key: Option<PathBuf>,
    pub(crate) command: Cmd,
}

impl Cli {
    pub(crate) fn parse() -> Self {
        match Self::try_parse_from(std::env::args_os()) {
            Ok(cli) => cli,
            Err(error) => {
                if error.code == 0 {
                    print!("{}", error.message);
                } else {
                    eprint!("{}", error.message);
                }
                std::process::exit(error.code);
            }
        }
    }

    pub(crate) fn paths(&self) -> anyhow::Result<plain_server::platform::system::consts::AppPaths> {
        let mut paths = plain_server::platform::system::consts::AppPaths::detect_with_directories(
            self.data_dir.clone(),
            self.cache_dir.clone(),
        )
        .map_err(anyhow::Error::msg)?;
        if let Some(path) = &self.tls_cert {
            paths.tls_cert = path.clone();
        }
        if let Some(path) = &self.tls_key {
            paths.tls_key = path.clone();
        }
        paths.data_dir = std::path::absolute(paths.data_dir)?;
        paths.cache_dir = std::path::absolute(paths.cache_dir)?;
        paths.tls_cert = std::path::absolute(paths.tls_cert)?;
        paths.tls_key = std::path::absolute(paths.tls_key)?;
        Ok(paths)
    }
}

#[derive(Debug)]
pub(crate) enum Cmd {
    Install {
        with_libreoffice: bool,
    },
    Uninstall {
        keep_config: bool,
        keep_data: bool,
    },
    Run {
        media_directories: Vec<PathBuf>,
        web_root: Option<PathBuf>,
        automount: bool,
    },
    Passwd {
        stdin: bool,
    },
    Login {
        command: login::Command,
    },
    Update,
    Bench,
}

#[cfg(test)]
#[path = "../../tests/unit/cli.rs"]
mod tests;
