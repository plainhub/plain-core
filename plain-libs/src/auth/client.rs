use super::{CLIENT_IDENTITY, Suite, api_key, auth_aad, context, ksf};
use opaque_ke::{ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers};
use rand_core::{OsRng, RngCore};
use zeroize::{Zeroize, Zeroizing};
pub struct OpaqueLogin {
    state: ClientLogin<Suite>,
    password: Zeroizing<Vec<u8>>,
    request: Vec<u8>,
}

impl OpaqueLogin {
    pub fn start(password: Vec<u8>) -> Result<OpaqueLogin, String> {
        let password = Zeroizing::new(password);
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut [0u8; 1])
            .map_err(|_| "Secure randomness unavailable")?;
        let started = ClientLogin::<Suite>::start(&mut rng, &password)
            .map_err(|_| String::from("Unable to start password login"))?;
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
        expected_server_key: &[u8],
    ) -> Result<LoginResult, String> {
        if server_signing_key.len() != 32 {
            return Err(String::from("invalid_auth_response"));
        }
        let response = CredentialResponse::<Suite>::deserialize(response)
            .map_err(|_| String::from("invalid_auth_response"))?;
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut [0u8; 1])
            .map_err(|_| "Secure randomness unavailable")?;
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
            .map_err(|_| String::from("invalid_password"))?;
        if finished.server_s_pk.serialize().as_slice() != expected_server_key {
            finished.session_key.zeroize();
            finished.export_key.zeroize();
            return Err("peer_identity_changed".into());
        }
        let result = LoginResult {
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

pub struct LoginResult {
    pub ke3: Vec<u8>,
    pub api_key: Zeroizing<Vec<u8>>,
    pub request_aad: Vec<u8>,
    pub response_aad: Vec<u8>,
    pub server_public_key: Vec<u8>,
}
