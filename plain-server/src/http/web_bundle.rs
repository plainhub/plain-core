use std::sync::Arc;

/// Read-only access to a frontend bundle that is compiled into the executable
/// rather than unpacked from a directory at startup.
pub trait WebBundle: Send + Sync {
    /// The bytes at `relative`, a `/`-separated path from the bundle root.
    fn read(&self, relative: &str) -> Option<&'static [u8]>;
}

pub type SharedWebBundle = Arc<dyn WebBundle>;

include!(concat!(env!("OUT_DIR"), "/web_bundle.rs"));

pub struct EmbeddedBundle;

impl WebBundle for EmbeddedBundle {
    fn read(&self, relative: &str) -> Option<&'static [u8]> {
        FILES
            .binary_search_by(|(key, _)| (*key).cmp(relative))
            .ok()
            .map(|index| FILES[index].1)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/http/web_bundle.rs"]
mod tests;
