use super::help::ParseError;
use super::parser::{Scope, text};
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    path::PathBuf,
};

#[derive(Default)]
pub(super) struct Values {
    pub(super) data_dir: Option<PathBuf>,
    pub(super) cache_dir: Option<PathBuf>,
    pub(super) tls_cert: Option<PathBuf>,
    pub(super) tls_key: Option<PathBuf>,
    pub(super) with_libreoffice: bool,
    pub(super) keep_config: bool,
    pub(super) keep_data: bool,
    pub(super) media_directories: Vec<PathBuf>,
    pub(super) web_root: Option<PathBuf>,
    pub(super) automount: bool,
    pub(super) stdin: bool,
    pub(super) after: Option<String>,
    pub(super) request_id: Option<String>,
}

impl Values {
    pub(super) fn apply(
        &mut self,
        name: &str,
        attached: Option<&OsStr>,
        args: &mut impl Iterator<Item = OsString>,
        scope: Scope,
        seen: &mut HashSet<(Scope, String)>,
    ) -> Result<(), ParseError> {
        let takes_value = match name {
            "--data-dir" | "--cache-dir" | "--tls-cert" | "--tls-key" => true,
            "--media-dir" | "--web-root" if scope == Scope::Run => true,
            "--after" if scope == Scope::Requests => true,
            "--with-libreoffice" if scope == Scope::Install => false,
            "--keep-config" | "--keep-data" if scope == Scope::Uninstall => false,
            "--automount" if scope == Scope::Run => false,
            "--stdin" if scope == Scope::Passwd => false,
            _ => return Err(ParseError::invalid(format!("unexpected argument {name:?}"))),
        };
        if name != "--media-dir" && !seen.insert((scope, name.to_owned())) {
            return Err(ParseError::invalid(format!(
                "argument {name:?} cannot be used multiple times"
            )));
        }
        let value = if takes_value {
            let value = attached
                .map(OsStr::to_owned)
                .or_else(|| args.next())
                .ok_or_else(|| ParseError::invalid(format!("a value is required for {name:?}")))?;
            if attached.is_none() && value != "-" && value.as_encoded_bytes().starts_with(b"-") {
                return Err(ParseError::invalid(format!(
                    "a value is required for {name:?}"
                )));
            }
            if value.is_empty() && name != "--after" {
                return Err(ParseError::invalid(format!(
                    "a nonempty value is required for {name:?}"
                )));
            }
            Some(value)
        } else {
            if attached.is_some() {
                return Err(ParseError::invalid(format!(
                    "{name:?} does not take a value"
                )));
            }
            None
        };
        match name {
            "--data-dir" => self.data_dir = value.map(PathBuf::from),
            "--cache-dir" => self.cache_dir = value.map(PathBuf::from),
            "--tls-cert" => self.tls_cert = value.map(PathBuf::from),
            "--tls-key" => self.tls_key = value.map(PathBuf::from),
            "--web-root" => self.web_root = value.map(PathBuf::from),
            "--media-dir" => self.media_directories.push(PathBuf::from(value.unwrap())),
            "--after" => self.after = Some(text(value.unwrap())?),
            "--with-libreoffice" => self.with_libreoffice = true,
            "--keep-config" => self.keep_config = true,
            "--keep-data" => self.keep_data = true,
            "--automount" => self.automount = true,
            "--stdin" => self.stdin = true,
            _ => unreachable!(),
        }
        Ok(())
    }
}
