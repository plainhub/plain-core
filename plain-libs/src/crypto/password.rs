use pbkdf2::{hmac::digest::CtOutput, sha2::Sha256};
use serde::{Deserialize, Serialize};

const ITERATIONS: u32 = 600_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Algorithm {
    Pbkdf2Sha256,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordHash {
    algorithm: Algorithm,
    iterations: u32,
    salt: [u8; 16],
    hash: [u8; 32],
}

impl PasswordHash {
    pub fn new(secret: &[u8]) -> Self {
        let mut salt = [0; 16];
        super::gen_random(&mut salt);
        Self {
            algorithm: Algorithm::Pbkdf2Sha256,
            iterations: ITERATIONS,
            salt,
            hash: pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(secret, &salt, ITERATIONS),
        }
    }

    pub fn verify(&self, secret: &[u8]) -> bool {
        if self.iterations != ITERATIONS {
            return false;
        }
        let actual = pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(secret, &self.salt, ITERATIONS);
        CtOutput::<Sha256>::new(actual.into()) == CtOutput::<Sha256>::new(self.hash.into())
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/crypto/password.rs"]
mod tests;
