use rustls::crypto::{CryptoProvider, ring};

pub fn builder() -> reqwest::ClientBuilder {
    // Match reqwest's Ring fallback when the host has not selected a provider.
    if CryptoProvider::get_default().is_none() {
        let _ = ring::default_provider().install_default();
    }
    reqwest::Client::builder()
}
