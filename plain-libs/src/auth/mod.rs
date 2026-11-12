use crate::aad::{AadPurpose, LoginPurpose};
use argon2::{Algorithm, Argon2, Params, Version};
use sha2::{Digest, Sha256};

pub const CLIENT_IDENTITY: &[u8] = b"web-admin";

pub fn ksf() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65536, 3, 4, None).expect("constant Argon2 parameters"),
    )
}

pub fn context(client_id: &str, server_key: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for part in [
        LoginPurpose::Password.as_str().as_bytes(),
        client_id.as_bytes(),
        server_key,
    ] {
        out.extend_from_slice(
            &u32::try_from(part.len())
                .expect("context length")
                .to_be_bytes(),
        );
        out.extend_from_slice(part);
    }
    out
}

pub fn api_key(session_key: &[u8], context: &[u8]) -> [u8; 32] {
    let salt = Sha256::digest(context);
    let mut key = [0; 32];
    hkdf::Hkdf::<Sha256>::new(Some(&salt), session_key)
        .expand(b"api", &mut key)
        .expect("fixed HKDF output length");
    key
}

pub fn auth_aad(context: &[u8], response: bool) -> Vec<u8> {
    let mut aad = if response {
        AadPurpose::AuthResponse.as_str().as_bytes().to_vec()
    } else {
        AadPurpose::AuthRequest.as_str().as_bytes().to_vec()
    };
    aad.extend_from_slice(&Sha256::digest(context));
    aad
}

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub mod server;

pub mod client;
