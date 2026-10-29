use super::help::ParseError;
use super::options::Values;
use super::{Cli, Cmd, login};
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Scope {
    Root,
    Install,
    Uninstall,
    Run,
    Passwd,
    Login,
    Requests,
    Allow,
    Deny,
    Update,
    Bench,
    RootHelp,
    LoginHelp,
}

impl Cli {
    pub(crate) fn try_parse_from(
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Result<Self, ParseError> {
        let mut args = args.into_iter().map(Into::into);
        args.next();
        let mut values = Values::default();
        let mut scope = Scope::Root;
        let mut seen = HashSet::new();
        let mut options = true;
        let mut help_command = false;
        while let Some(argument) = args.next() {
            if help_command && argument.as_encoded_bytes().starts_with(b"-") {
                return Err(ParseError::invalid("help expects a command name"));
            }
            if options && argument == "--" {
                options = false;
                continue;
            }
            let (name, attached) = split(&argument);
            if options && name.starts_with('-') {
                if name == "--help" || (name.starts_with("-h") && !name.starts_with("--")) {
                    if name == "--help" && attached.is_some() {
                        return Err(ParseError::invalid("help does not take a value"));
                    }
                    return Err(ParseError::help(scope));
                }
                if scope == Scope::Root
                    && (name == "--version" || (name.starts_with("-V") && !name.starts_with("--")))
                    && (name != "--version" || attached.is_none())
                {
                    return Err(ParseError {
                        code: 0,
                        message: format!("plainapp {}\n", env!("CARGO_PKG_VERSION")),
                    });
                }
                values.apply(name, attached, &mut args, scope, &mut seen)?;
            } else {
                if !options && matches!(scope, Scope::Root | Scope::Login) {
                    return Err(ParseError::invalid("a subcommand is required before --"));
                }
                let word = text(argument)?;
                scope = match (scope, word.as_str()) {
                    (Scope::Root | Scope::Login, "help") if options => {
                        if help_command {
                            if scope == Scope::Root {
                                Scope::RootHelp
                            } else {
                                Scope::LoginHelp
                            }
                        } else {
                            help_command = true;
                            scope
                        }
                    }
                    (Scope::Root, "install") => Scope::Install,
                    (Scope::Root, "uninstall") => Scope::Uninstall,
                    (Scope::Root, "run") => Scope::Run,
                    (Scope::Root, "passwd") => Scope::Passwd,
                    (Scope::Root, "login") => Scope::Login,
                    (Scope::Root, "update") => Scope::Update,
                    (Scope::Root, "bench") => Scope::Bench,
                    (Scope::Login, "requests") => Scope::Requests,
                    (Scope::Login, "allow") => Scope::Allow,
                    (Scope::Login, "deny") => Scope::Deny,
                    (Scope::Allow | Scope::Deny, _)
                        if !help_command && values.request_id.is_none() =>
                    {
                        values.request_id = Some(word);
                        scope
                    }
                    _ => return Err(ParseError::invalid(format!("unexpected argument {word:?}"))),
                };
            }
        }
        if help_command {
            return Err(ParseError::help(scope));
        }
        let command = match scope {
            Scope::Install => Cmd::Install {
                with_libreoffice: values.with_libreoffice,
            },
            Scope::Uninstall => Cmd::Uninstall {
                keep_config: values.keep_config,
                keep_data: values.keep_data,
            },
            Scope::Run => Cmd::Run {
                media_directories: values.media_directories,
                web_root: values.web_root,
                automount: values.automount,
            },
            Scope::Passwd => Cmd::Passwd {
                stdin: values.stdin,
            },
            Scope::Requests => Cmd::Login {
                command: login::Command::Requests {
                    after: values.after,
                },
            },
            Scope::Allow | Scope::Deny => {
                let request_id = values
                    .request_id
                    .ok_or_else(|| ParseError::invalid("a request ID is required"))?;
                Cmd::Login {
                    command: if scope == Scope::Allow {
                        login::Command::Allow { request_id }
                    } else {
                        login::Command::Deny { request_id }
                    },
                }
            }
            Scope::Update => Cmd::Update,
            Scope::Bench => Cmd::Bench,
            Scope::Root | Scope::Login | Scope::RootHelp | Scope::LoginHelp => {
                return Err(ParseError {
                    code: 2,
                    message: super::help::render(scope).to_owned(),
                });
            }
        };
        Ok(Self {
            data_dir: values.data_dir,
            cache_dir: values.cache_dir,
            tls_cert: values.tls_cert,
            tls_key: values.tls_key,
            command,
        })
    }
}

pub(super) fn text(value: OsString) -> Result<String, ParseError> {
    value
        .into_string()
        .map_err(|_| ParseError::invalid("argument is not valid UTF-8"))
}

fn split(value: &OsStr) -> (&str, Option<&OsStr>) {
    let bytes = value.as_encoded_bytes();
    if let Some(index) = bytes.iter().position(|byte| *byte == b'=') {
        if let Ok(name) = std::str::from_utf8(&bytes[..index]) {
            // An ASCII '=' is a valid boundary in the platform's self-synchronizing encoding.
            return (
                name,
                Some(unsafe { OsStr::from_encoded_bytes_unchecked(&bytes[index + 1..]) }),
            );
        }
    }
    (value.to_str().unwrap_or(""), None)
}

#[cfg(test)]
#[path = "../../tests/unit/cli/parser.rs"]
mod tests;
