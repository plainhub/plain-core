use super::client::{self, LoginError};
use wasm_bindgen::prelude::*;

fn javascript_error(error: LoginError) -> JsValue {
    JsValue::from_str(match error {
        LoginError::Randomness => "Secure randomness is unavailable",
        LoginError::Start => "Unable to start password login",
        LoginError::SigningKey => "Invalid server signing key",
        LoginError::Response => "Invalid password login response",
        LoginError::Password => "Password login failed",
        LoginError::PeerIdentity => "Peer identity changed",
    })
}

#[wasm_bindgen]
pub struct OpaqueLogin {
    inner: client::OpaqueLogin,
}
#[wasm_bindgen]
impl OpaqueLogin {
    pub fn start(password: Vec<u8>) -> Result<OpaqueLogin, JsValue> {
        client::OpaqueLogin::try_start(password)
            .map(|inner| Self { inner })
            .map_err(javascript_error)
    }
    pub fn request(&self) -> Vec<u8> {
        self.inner.request()
    }
    pub fn finish(
        self,
        response: &[u8],
        cid: &str,
        server_signing_key: &[u8],
    ) -> Result<OpaqueLoginResult, JsValue> {
        self.inner
            .finish_protocol(response, cid, server_signing_key, None)
            .map(|inner| OpaqueLoginResult { inner })
            .map_err(javascript_error)
    }
}
#[wasm_bindgen]
pub struct OpaqueLoginResult {
    inner: client::LoginResult,
}
#[wasm_bindgen]
impl OpaqueLoginResult {
    #[wasm_bindgen(getter)]
    pub fn ke3(&self) -> Vec<u8> {
        self.inner.ke3.clone()
    }
    #[wasm_bindgen(getter, js_name = apiKey)]
    pub fn api_key(&self) -> Vec<u8> {
        self.inner.api_key.to_vec()
    }
    #[wasm_bindgen(getter, js_name = requestAad)]
    pub fn request_aad(&self) -> Vec<u8> {
        self.inner.request_aad.clone()
    }
    #[wasm_bindgen(getter, js_name = responseAad)]
    pub fn response_aad(&self) -> Vec<u8> {
        self.inner.response_aad.clone()
    }
    #[wasm_bindgen(getter, js_name = serverPublicKey)]
    pub fn server_public_key(&self) -> Vec<u8> {
        self.inner.server_public_key.clone()
    }
}
