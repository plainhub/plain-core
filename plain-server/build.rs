#[path = "build/web_assets.rs"]
mod web_assets;

fn main() {
    println!("cargo:rerun-if-env-changed=PLAIN_WEB_DIST");
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let configured = std::env::var_os("PLAIN_WEB_DIST");
    let root = configured
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| manifest.join("../assets/web"));
    assert!(
        root.is_absolute(),
        "PLAIN_WEB_DIST must be an absolute directory"
    );
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("web_bundle.rs");
    web_assets::generate_with_icons(
        &root,
        &manifest.join("../assets/ficons"),
        &output,
        configured.is_some(),
    )
    .expect("Cannot embed Web assets");
}
