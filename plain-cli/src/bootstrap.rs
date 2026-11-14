use plain_libs::anyhow::Result;

pub fn run() -> Result<()> {
    initialize_tls();

    plain_rs::infrastructure::platform::version::set(
        crate::version::VERSION,
        crate::version::COMMIT,
        crate::version::BUILD_TIME,
    );
    plain_rs::infrastructure::platform::system::log::init("info").map_err(plain_libs::anyhow::Error::msg)?;

    let cli = crate::cli::Cli::parse();
    let paths = cli.paths()?;

    match cli.command {
        crate::cli::Cmd::Install => crate::cli::install::run(&paths),
        crate::cli::Cmd::Uninstall {
            keep_config,
            keep_data,
        } => crate::cli::uninstall::run(&paths, keep_config, keep_data),
        crate::cli::Cmd::Passwd { stdin } => crate::cli::passwd::run(&paths, stdin),
        crate::cli::Cmd::Login { command } => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(crate::cli::login::run(&paths, command)),
        crate::cli::Cmd::Prefs { key, value } => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(crate::cli::prefs::run(&paths, &key, &value)),
        crate::cli::Cmd::Update => crate::cli::update::run(&paths),
        crate::cli::Cmd::Run {
            media_directories,
            web_root,
        } => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(crate::cli::run::run(
                &paths,
                crate::service::Options {
                    #[cfg(test)]
                    file_index_defaults: None,
                    media_directories,
                    web_root,
                },
            ))
        }
    }
}

pub(crate) fn initialize_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
