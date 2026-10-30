use super::*;

struct Stub(&'static [(&'static str, &'static [u8])]);

impl WebBundle for Stub {
    fn read(&self, relative: &str) -> Option<&'static [u8]> {
        self.0
            .iter()
            .find(|(key, _)| *key == relative)
            .map(|(_, bytes)| *bytes)
    }
}

#[test]
fn a_bundle_is_reached_through_a_shared_handle() {
    let bundle: SharedWebBundle = std::sync::Arc::new(Stub(&[("index.html", b"shell")]));
    assert_eq!(bundle.read("index.html"), Some(&b"shell"[..]));
    assert_eq!(bundle.read("missing.js"), None);
}

#[path = "../../../build/web_assets.rs"]
mod build_web_assets;

#[test]
fn compiled_assets_use_sorted_relative_keys_and_preserve_bytes() {
    assert!(FILES.windows(2).all(|pair| pair[0].0 < pair[1].0));
    let bundle = EmbeddedBundle;
    for (path, bytes) in FILES {
        assert_eq!(bundle.read(path), Some(*bytes));
        assert!(!path.starts_with('/'));
        assert!(!path.contains('\\'));
    }
    assert_eq!(bundle.read("missing-file.js"), None);
    assert_eq!(bundle.read("../index.html"), None);
}
