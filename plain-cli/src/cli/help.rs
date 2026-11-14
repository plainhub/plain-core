use super::parser::Scope;

pub(super) fn render(scope: Scope) -> &'static str {
    match scope {
        Scope::Root => {
            "PlainApp server and command-line tools\n\nUsage: plainapp [OPTIONS] <COMMAND>\n\nCommands:\n  install    \n  uninstall  \n  run        \n  passwd     \n  login      \n  update     \n  dev        Enable or disable developer mode\n  help       Print this message or the help of the given subcommand(s)\n\nOptions:\n      --data-dir <DATA_DIR>    \n      --cache-dir <CACHE_DIR>  \n  -h, --help                   Print help\n  -V, --version                Print version\n"
        }
        Scope::Install => {
            "Usage: plainapp install [OPTIONS]\n\nOptions:\n  -h, --help                   Print help\n"
        }
        Scope::Uninstall => {
            "Usage: plainapp uninstall [OPTIONS]\n\nOptions:\n      --data-dir <DATA_DIR>    \n      --keep-config            \n      --keep-data              \n  -h, --help                   Print help\n"
        }
        Scope::Run => {
            "Usage: plainapp run [OPTIONS]\n\nOptions:\n      --data-dir <DATA_DIR>            \n      --media-dir <MEDIA_DIRECTORIES>  \n      --cache-dir <CACHE_DIR>          \n      --web-root <WEB_ROOT>            \n  -h, --help                           Print help\n"
        }
        Scope::Passwd => {
            "Usage: plainapp passwd [OPTIONS]\n\nOptions:\n      --data-dir <DATA_DIR>    \n      --stdin                  Read the new password from standard input\n      --cache-dir <CACHE_DIR>  \n  -h, --help                   Print help\n"
        }
        Scope::Login => {
            "Usage: plainapp login [OPTIONS] <COMMAND>\n\nCommands:\n  requests  \n  allow     \n  deny      \n  help      Print this message or the help of the given subcommand(s)\n\nOptions:\n      --data-dir <DATA_DIR>    \n  -h, --help                   Print help\n"
        }
        Scope::Requests => {
            "Usage: plainapp login requests [OPTIONS]\n\nOptions:\n      --after <AFTER>          \n      --data-dir <DATA_DIR>    \n  -h, --help                   Print help\n"
        }
        Scope::Allow => {
            "Usage: plainapp login allow [OPTIONS] <REQUEST_ID>\n\nArguments:\n  <REQUEST_ID>  \n\nOptions:\n      --data-dir <DATA_DIR>    \n  -h, --help                   Print help\n"
        }
        Scope::Deny => {
            "Usage: plainapp login deny [OPTIONS] <REQUEST_ID>\n\nArguments:\n  <REQUEST_ID>  \n\nOptions:\n      --data-dir <DATA_DIR>    \n  -h, --help                   Print help\n"
        }
        Scope::Update => {
            "Usage: plainapp update [OPTIONS]\n\nOptions:\n  -h, --help                   Print help\n"
        }
        Scope::Dev => {
            "Usage: plainapp dev [OPTIONS] <COMMAND>\n\nCommands:\n  on   Enable developer mode\n  off  Disable developer mode\n\nOptions:\n      --data-dir <DATA_DIR>    \n      --cache-dir <CACHE_DIR>  \n  -h, --help                   Print help\n"
        }
        Scope::DevOn => {
            "Usage: plainapp dev on [OPTIONS]\n\nOptions:\n      --data-dir <DATA_DIR>    \n      --cache-dir <CACHE_DIR>  \n  -h, --help                   Print help\n"
        }
        Scope::DevOff => {
            "Usage: plainapp dev off [OPTIONS]\n\nOptions:\n      --data-dir <DATA_DIR>    \n      --cache-dir <CACHE_DIR>  \n  -h, --help                   Print help\n"
        }
        Scope::RootHelp => {
            "Print this message or the help of the given subcommand(s)\n\nUsage: plainapp help [COMMAND]...\n\nArguments:\n  [COMMAND]...  Print help for the subcommand(s)\n"
        }
        Scope::LoginHelp => {
            "Print this message or the help of the given subcommand(s)\n\nUsage: plainapp login help [COMMAND]...\n\nArguments:\n  [COMMAND]...  Print help for the subcommand(s)\n"
        }
    }
}

#[derive(Debug)]
pub(crate) struct ParseError {
    pub(crate) code: i32,
    pub(crate) message: String,
}

impl ParseError {
    pub(super) fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: format!(
                "error: {}\n\nFor more information, try '--help'.\n",
                message.into()
            ),
        }
    }

    pub(super) fn help(scope: Scope) -> Self {
        Self {
            code: 0,
            message: super::help::render(scope).to_owned(),
        }
    }
}
