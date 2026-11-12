// Copyright (c) Meta Platforms, Inc. and affiliates; adapted under the retained source licenses.
use super::*;

pub(super) fn expand(key: &[u8], info: &[&[u8]], output: &mut [u8]) -> Result<(), ProtocolError> {
    Hkdf::<Sha512>::from_prk(key)
        .map_err(|_| ProtocolError::LibraryError)?
        .expand_multi_info(info, output)
        .map_err(|_| ProtocolError::LibraryError)
}
pub(super) fn randomized_password(
    blind: &Blind,
    password: &[u8],
    evaluation: RistrettoPoint,
    ksf: Option<&Argon2<'_>>,
) -> Result<Hkdf<Sha512>, ProtocolError> {
    let output = blind.finalize(password, evaluation)?;
    let mut hardened = Zeroizing::new([0; 64]);
    ksf.unwrap_or(&Argon2::default())
        .hash_password_into(
            &output[..],
            &[0; argon2::RECOMMENDED_SALT_LEN],
            &mut hardened[..],
        )
        .map_err(|_| ProtocolError::LibraryError)?;
    let mut extract = HkdfExtract::<Sha512>::new(None);
    extract.input_ikm(&output[..]);
    extract.input_ikm(&hardened[..]);
    Ok(extract.finalize().1)
}
pub(super) fn client_static_key(
    password_key: &Hkdf<Sha512>,
    nonce: &[u8; 32],
) -> Result<NonZeroScalar, ProtocolError> {
    let mut seed = Zeroizing::new([0; 32]);
    password_key
        .expand_multi_info(&[nonce, b"PrivateKey"], &mut seed[..])
        .map_err(|_| ProtocolError::LibraryError)?;
    oprf::derive_key(&seed[..], b"OPAQUE-DeriveDiffieHellmanKeyPair")
}
fn length(bytes: &[u8]) -> Result<[u8; 2], ProtocolError> {
    Ok(u16::try_from(bytes.len())
        .map_err(|_| ProtocolError::SerializationError)?
        .to_be_bytes())
}
pub(super) fn envelope_mac(
    password_key: &Hkdf<Sha512>,
    nonce: &[u8; 32],
    client: PublicKey,
    server: PublicKey,
    identifiers: Identifiers<'_>,
) -> Result<[u8; 64], ProtocolError> {
    let mut key = Zeroizing::new([0; 64]);
    password_key
        .expand_multi_info(&[nonce, b"AuthKey"], &mut key[..])
        .map_err(|_| ProtocolError::LibraryError)?;
    let mut mac =
        Hmac::<Sha512>::new_from_slice(&key[..]).map_err(|_| ProtocolError::LibraryError)?;
    let server_bytes = server.serialize();
    let client_id = Identifier::new(identifiers.client, || client.serialize());
    let server_id = Identifier::new(identifiers.server, || server_bytes);
    let client = client_id.as_ref();
    let server = server_id.as_ref();
    for part in [
        nonce.as_slice(),
        &server_bytes,
        &length(server)?,
        server,
        &length(client)?,
        client,
    ] {
        Mac::update(&mut mac, part);
    }
    Ok(mac.finalize().into_bytes().into())
}
pub(super) fn transcript(
    context: Option<&[u8]>,
    identifiers: Identifiers<'_>,
    client: PublicKey,
    server: PublicKey,
    request: &CredentialRequest,
    response: &CredentialResponse,
) -> Result<Sha512, ProtocolError> {
    let context = context.unwrap_or_default();
    let client_id = Identifier::new(identifiers.client, || client.serialize());
    let server_id = Identifier::new(identifiers.server, || server.serialize());
    let client = client_id.as_ref();
    let server = server_id.as_ref();
    let mut hash = Sha512::new();
    for part in [
        b"OPAQUEv1-".as_slice(),
        &length(context)?,
        context,
        &length(client)?,
        client,
        &request.serialize(),
        &length(server)?,
        server,
        &response.evaluation.compress().to_bytes(),
        &response.masking_nonce,
        &response.masked,
        &response.nonce,
        &response.public.serialize(),
    ] {
        hash.update(part);
    }
    Ok(hash)
}
fn expand_label(
    hkdf: &Hkdf<Sha512>,
    label: &[u8],
    context: &[u8],
) -> Result<Zeroizing<[u8; 64]>, ProtocolError> {
    let mut result = Zeroizing::new([0; 64]);
    hkdf.expand_multi_info(
        &[
            &[0, 64],
            &[(7 + label.len()) as u8],
            b"OPAQUE-",
            label,
            &[context.len() as u8],
            context,
        ],
        &mut result[..],
    )
    .map_err(|_| ProtocolError::LibraryError)?;
    Ok(result)
}
pub(super) struct DerivedKeys {
    pub(super) session: Zeroizing<[u8; 64]>,
    handshake: Zeroizing<[u8; 64]>,
}
impl DerivedKeys {
    pub(super) fn mac(&self, label: &[u8], digest: &[u8]) -> Result<[u8; 64], ProtocolError> {
        let hkdf = Hkdf::<Sha512>::from_prk(&self.handshake[..])
            .map_err(|_| ProtocolError::LibraryError)?;
        let key = expand_label(&hkdf, label, b"")?;
        let mut mac =
            Hmac::<Sha512>::new_from_slice(&key[..]).map_err(|_| ProtocolError::LibraryError)?;
        Mac::update(&mut mac, digest);
        Ok(mac.finalize().into_bytes().into())
    }
}
pub(super) fn derive_keys(secrets: &[&[u8]], digest: &[u8]) -> Result<DerivedKeys, ProtocolError> {
    let mut extract = HkdfExtract::<Sha512>::new(None);
    for secret in secrets {
        extract.input_ikm(secret);
    }
    let hkdf = extract.finalize().1;
    Ok(DerivedKeys {
        session: expand_label(&hkdf, b"SessionKey", digest)?,
        handshake: expand_label(&hkdf, b"HandshakeSecret", digest)?,
    })
}

pub(super) enum Identifier<'a> {
    Borrowed(&'a [u8]),
    Encoded([u8; 32]),
}
impl<'a> Identifier<'a> {
    #[inline]
    pub(super) fn new(id: Option<&'a [u8]>, encode: impl FnOnce() -> [u8; 32]) -> Self {
        match id {
            Some(bytes) => Self::Borrowed(bytes),
            None => Self::Encoded(encode()),
        }
    }
}
impl AsRef<[u8]> for Identifier<'_> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Encoded(bytes) => bytes,
        }
    }
}
