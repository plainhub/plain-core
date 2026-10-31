use std::sync::OnceLock;

static VERSION: OnceLock<String> = OnceLock::new();
static FULL_VERSION: OnceLock<String> = OnceLock::new();

pub fn set_version(version: &str) {
    let _ = VERSION.set(version.to_owned());
}

pub fn set(version: &str, commit: &str, build_time: &str) {
    set_version(version);
    let _ = FULL_VERSION.set(format!("{version} (commit {commit}, built {build_time})"));
}

pub fn version() -> &'static str {
    VERSION
        .get()
        .map(String::as_str)
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

pub fn full_version() -> String {
    FULL_VERSION
        .get()
        .cloned()
        .unwrap_or_else(|| version().to_owned())
}
