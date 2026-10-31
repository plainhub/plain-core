use std::path::PathBuf;

#[derive(Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub tls_cert: PathBuf,
    pub tls_key: PathBuf,
}

impl AppPaths {
    #[doc(hidden)]
    pub fn pin_test_data_dir() -> PathBuf {
        static PINNED: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        PINNED
            .get_or_init(|| {
                let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-data-dir");
                std::fs::create_dir_all(&dir).expect("mkdir target/test-data-dir");
                unsafe { std::env::set_var("PLAINAPP_DATA_DIR", &dir) };
                dir
            })
            .clone()
    }

    pub fn detect() -> Result<Self, String> {
        Self::detect_with_data_directory(None)
    }

    pub fn detect_with_data_directory(directory: Option<PathBuf>) -> Result<Self, String> {
        Self::detect_with_directories(directory, None)
    }
    pub fn detect_with_directories(
        directory: Option<PathBuf>,
        cache: Option<PathBuf>,
    ) -> Result<Self, String> {
        let base = home_directory()
            .unwrap_or_else(std::env::temp_dir)
            .join(".plainapp");
        let data_dir = directory
            .or_else(|| configured_path("PLAINAPP_DATA_DIR"))
            .unwrap_or_else(|| base.join("data"));
        let data_dir = std::path::absolute(data_dir).map_err(|error| error.to_string())?;
        let cache_dir = match cache.or_else(|| configured_path("PLAINAPP_CACHE_DIR")) {
            Some(cache) => std::path::absolute(cache).map_err(|error| error.to_string())?,
            None => plain_rs::cache_paths::system::resolve(&data_dir)?,
        };
        Ok(Self {
            cache_dir,
            tls_cert: configured_path("PLAINAPP_TLS_CERT")
                .unwrap_or_else(|| data_dir.join("tls.pem")),
            tls_key: configured_path("PLAINAPP_TLS_KEY")
                .unwrap_or_else(|| data_dir.join("tls.key")),
            data_dir,
        })
    }
}

fn configured_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

pub fn home_directory() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
