// Copyright (c) Meta Platforms, Inc. and affiliates; adapted under the retained source licenses.
#![forbid(unsafe_code)]
// Adapted from voprf0.5.0 Ristretto base mode and elliptic-curve0.13.8 XMD; see SOURCE.json.
use crate::opaque::errors::ProtocolError;
use curve25519_dalek::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::IsIdentity,
};
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha512};
use std::ops::Deref;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

#[repr(transparent)]
pub(crate) struct NonZeroScalar(Zeroizing<Scalar>);
impl Deref for NonZeroScalar {
    type Target = Scalar;
    #[inline]
    fn deref(&self) -> &Scalar {
        &self.0
    }
}

const CONTEXT: &[u8] = b"OPRFV1-\0-ristretto255-SHA512";

// This fixed suite requires exactly one SHA-512 output block and a short, fixed DST.
fn expand_message(input: &[&[u8]], label: &[u8]) -> [u8; 64] {
    let dst_len = [(label.len() + CONTEXT.len()) as u8];
    let mut hash = Sha512::new();
    hash.update([0; 128]);
    for part in input {
        hash.update(part);
    }
    hash.update([0, 64, 0]);
    hash.update(label);
    hash.update(CONTEXT);
    hash.update(dst_len);
    let b0 = Zeroizing::new(<[u8; 64]>::from(hash.finalize()));
    let mut hash = Sha512::new();
    hash.update(&b0[..]);
    hash.update([1]);
    hash.update(label);
    hash.update(CONTEXT);
    hash.update(dst_len);
    hash.finalize().into()
}

pub(crate) fn derive_key(seed: &[u8], info: &[u8]) -> Result<NonZeroScalar, ProtocolError> {
    let len = u16::try_from(info.len())
        .map_err(|_| ProtocolError::SerializationError)?
        .to_be_bytes();
    for counter in 0..=u8::MAX {
        let uniform = Zeroizing::new(expand_message(
            &[seed, &len, info, &[counter]],
            b"DeriveKeyPair",
        ));
        let scalar = Zeroizing::new(Scalar::from_bytes_mod_order_wide(&uniform));
        if !bool::from(scalar.ct_eq(&Scalar::ZERO)) {
            return Ok(NonZeroScalar(scalar));
        }
    }
    Err(ProtocolError::SerializationError)
}

pub(crate) fn random_scalar(rng: &mut (impl RngCore + CryptoRng)) -> NonZeroScalar {
    loop {
        let scalar = Zeroizing::new(Scalar::random(rng));
        if !bool::from(scalar.ct_eq(&Scalar::ZERO)) {
            return NonZeroScalar(scalar);
        }
    }
}

pub(crate) fn public_key(scalar: &NonZeroScalar) -> RistrettoPoint {
    RistrettoPoint::mul_base(scalar)
}

pub(crate) fn decode_point(bytes: [u8; 32]) -> Result<RistrettoPoint, ProtocolError> {
    CompressedRistretto(bytes)
        .decompress()
        .filter(|point| !point.is_identity())
        .ok_or(ProtocolError::SerializationError)
}

pub(crate) fn decode_scalar(bytes: [u8; 32]) -> Result<NonZeroScalar, ProtocolError> {
    let scalar = Option::<Scalar>::from(Scalar::from_canonical_bytes(bytes))
        .ok_or(ProtocolError::SerializationError)?;
    if bool::from(scalar.ct_eq(&Scalar::ZERO)) {
        return Err(ProtocolError::SerializationError);
    }
    Ok(NonZeroScalar(Zeroizing::new(scalar)))
}

pub(crate) struct Blind {
    scalar: NonZeroScalar,
}
impl Blind {
    pub(crate) fn new(
        input: &[u8],
        rng: &mut (impl RngCore + CryptoRng),
    ) -> (Self, RistrettoPoint) {
        let scalar = random_scalar(rng);
        let uniform = Zeroizing::new(expand_message(&[input], b"HashToGroup-"));
        let element = RistrettoPoint::from_uniform_bytes(&uniform) * *scalar;
        (Self { scalar }, element)
    }
    pub(crate) fn finalize(
        &self,
        input: &[u8],
        evaluation: RistrettoPoint,
    ) -> Result<Zeroizing<[u8; 64]>, ProtocolError> {
        let len = u16::try_from(input.len())
            .map_err(|_| ProtocolError::LibraryError)?
            .to_be_bytes();
        let inverse = Zeroizing::new(self.scalar.invert());
        let unblinded = Zeroizing::new((evaluation * *inverse).compress().to_bytes());
        let mut hash = Sha512::new();
        hash.update(len);
        hash.update(input);
        hash.update([0, 32]);
        hash.update(&unblinded[..]);
        hash.update(b"Finalize");
        Ok(Zeroizing::new(hash.finalize().into()))
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/oprf.rs"]
mod tests;
