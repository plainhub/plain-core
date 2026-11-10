use opaque_ke::{ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers};
use rand_core::{OsRng, RngCore};
use wasm_bindgen::prelude::*;
use zeroize::{Zeroize, Zeroizing};

use super::{CLIENT_IDENTITY, Suite, api_key, auth_aad, context, ksf};

#[wasm_bindgen]
pub struct OpaqueLogin {
    state: ClientLogin<Suite>,
    password: Zeroizing<Vec<u8>>,
    request: Vec<u8>,
}

#[wasm_bindgen]
impl OpaqueLogin {
    pub fn start(password: Vec<u8>) -> Result<OpaqueLogin, JsValue> {
        let password = Zeroizing::new(password);
        let mut rng = secure_rng()?;
        let started = ClientLogin::<Suite>::start(&mut rng, &password)
            .map_err(|_| JsValue::from_str("Unable to start password login"))?;
        Ok(Self {
            state: started.state,
            password,
            request: started.message.serialize().to_vec(),
        })
    }

    pub fn request(&self) -> Vec<u8> {
        self.request.clone()
    }

    pub fn finish(
        self,
        response: &[u8],
        cid: &str,
        server_signing_key: &[u8],
    ) -> Result<OpaqueLoginResult, JsValue> {
        if server_signing_key.len() != 32 {
            return Err(JsValue::from_str("Invalid server signing key"));
        }
        let response = CredentialResponse::<Suite>::deserialize(response)
            .map_err(|_| JsValue::from_str("Invalid password login response"))?;
        let mut rng = secure_rng()?;
        let context = context(cid, server_signing_key);
        let ksf = ksf();
        let mut finished = self
            .state
            .finish(
                &mut rng,
                &self.password,
                response,
                ClientLoginFinishParameters::new(
                    Some(&context),
                    Identifiers {
                        client: Some(CLIENT_IDENTITY),
                        server: Some(server_signing_key),
                    },
                    Some(&ksf),
                ),
            )
            .map_err(|_| JsValue::from_str("Password login failed"))?;
        let result = OpaqueLoginResult {
            ke3: finished.message.serialize().to_vec(),
            api_key: Zeroizing::new(api_key(&finished.session_key, &context).to_vec()),
            request_aad: auth_aad(&context, false),
            response_aad: auth_aad(&context, true),
            server_public_key: finished.server_s_pk.serialize().to_vec(),
        };
        finished.session_key.zeroize();
        finished.export_key.zeroize();
        Ok(result)
    }
}

#[wasm_bindgen]
pub struct OpaqueLoginResult {
    ke3: Vec<u8>,
    api_key: Zeroizing<Vec<u8>>,
    request_aad: Vec<u8>,
    response_aad: Vec<u8>,
    server_public_key: Vec<u8>,
}

#[wasm_bindgen]
impl OpaqueLoginResult {
    #[wasm_bindgen(getter)]
    pub fn ke3(&self) -> Vec<u8> {
        self.ke3.clone()
    }

    #[wasm_bindgen(getter, js_name = apiKey)]
    pub fn api_key(&self) -> Vec<u8> {
        self.api_key.to_vec()
    }

    #[wasm_bindgen(getter, js_name = requestAad)]
    pub fn request_aad(&self) -> Vec<u8> {
        self.request_aad.clone()
    }

    #[wasm_bindgen(getter, js_name = responseAad)]
    pub fn response_aad(&self) -> Vec<u8> {
        self.response_aad.clone()
    }

    #[wasm_bindgen(getter, js_name = serverPublicKey)]
    pub fn server_public_key(&self) -> Vec<u8> {
        self.server_public_key.clone()
    }
}

fn secure_rng() -> Result<OsRng, JsValue> {
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut [0u8; 1])
        .map_err(|_| JsValue::from_str("Secure randomness is unavailable"))?;
    Ok(rng)
}
