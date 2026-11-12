use super::{CLIENT_IDENTITY, api_key, auth_aad, context, ksf};
use crate::opaque::{ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers};
use rand_core::{OsRng, RngCore};
use zeroize::{Zeroize, Zeroizing};
pub struct OpaqueLogin {
    state: ClientLogin,
    password: Zeroizing<Vec<u8>>,
    request: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum LoginError {
    Randomness,
    Start,
    SigningKey,
    Response,
    Password,
    PeerIdentity,
}
impl LoginError {
    fn native_message(self) -> String {
        match self {
            Self::Randomness => "Secure randomness unavailable",
            Self::Start => "Unable to start password login",
            Self::SigningKey | Self::Response => "invalid_auth_response",
            Self::Password => "invalid_password",
            Self::PeerIdentity => "peer_identity_changed",
        }
        .into()
    }
}
impl OpaqueLogin {
    pub fn start(password: Vec<u8>) -> Result<OpaqueLogin, String> {
        Self::try_start(password).map_err(LoginError::native_message)
    }
    pub(crate) fn try_start(password: Vec<u8>) -> Result<OpaqueLogin, LoginError> {
        let password = Zeroizing::new(password);
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut [0u8; 1])
            .map_err(|_| LoginError::Randomness)?;
        let started = ClientLogin::start(&mut rng, &password).map_err(|_| LoginError::Start)?;
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
        self.finish_protocol(response, cid, server_signing_key, Some(expected_server_key))
            .map_err(LoginError::native_message)
    }
    pub(crate) fn finish_protocol(
        self,
        response: &[u8],
        cid: &str,
        server_signing_key: &[u8],
        expected_server_key: Option<&[u8]>,
    ) -> Result<LoginResult, LoginError> {
        if server_signing_key.len() != 32 {
            return Err(LoginError::SigningKey);
        }
        let response =
            CredentialResponse::deserialize(response).map_err(|_| LoginError::Response)?;
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut [0u8; 1])
            .map_err(|_| LoginError::Randomness)?;
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
            .map_err(|_| LoginError::Password)?;
        if expected_server_key
            .is_some_and(|expected| finished.server_s_pk.serialize().as_slice() != expected)
        {
            finished.session_key.zeroize();
            finished.export_key.zeroize();
            return Err(LoginError::PeerIdentity);
        }
        let result = LoginResult {
            ke3: finished.message.serialize().to_vec(),
            api_key: Zeroizing::new(api_key(finished.session_key.as_slice(), &context).to_vec()),
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
