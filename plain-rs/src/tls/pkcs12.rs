use der::{
    Any, Decode, Encode,
    asn1::{BmpString, ContextSpecific, ObjectIdentifier, OctetString},
};
use hmac::{KeyInit, Mac};
use pkcs12::{
    AuthenticatedSafe, CertBag, Pfx, SafeBag,
    cms::{
        content_info::{CmsVersion, ContentInfo},
        encrypted_data::EncryptedData,
    },
    kdf::{Pkcs12KeyType, derive_key_utf8},
    pbe_params::{EncryptedPrivateKeyInfo, Pkcs12PbeParams},
};
use std::fmt::Display;

type Result<T> = std::result::Result<T, String>;
const DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
const ENCRYPTED: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.6");
const PBES2: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.5.13");
const LOCAL_ID: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.21");
const NAME: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.20");
const TRUST: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.113894.746875.1.1");
const ANY_USAGE: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.37.0");
const MAX_ITERATIONS: i32 = 1_000_000;

struct Key {
    id: Vec<u8>,
    alias: Option<String>,
    der: Vec<u8>,
}

struct Certificate {
    id: Option<Vec<u8>>,
    der: Vec<u8>,
    subject: String,
    issuer: String,
    trusted: bool,
}

fn error(error: impl Display) -> String {
    error.to_string()
}

fn iterations(count: i32) -> Result<()> {
    if !(1..=MAX_ITERATIONS).contains(&count) {
        return Err("PKCS12 iteration count out of range".into());
    }
    Ok(())
}

fn authenticate(mac: &pkcs12::MacData, password: &str, bytes: &[u8]) -> Result<()> {
    iterations(mac.iterations)?;
    macro_rules! digest {
        ($hash:ty) => {{
            let size = <$hash as hmac::digest::OutputSizeUser>::output_size();
            let key = derive_key_utf8::<$hash>(
                password,
                mac.mac_salt.as_bytes(),
                Pkcs12KeyType::Mac,
                mac.iterations,
                size,
            )
            .map_err(error)?;
            let mut verifier = hmac::Hmac::<$hash>::new_from_slice(&key).map_err(error)?;
            verifier.update(bytes);
            verifier
                .verify_slice(mac.mac.digest.as_bytes())
                .map_err(|_| "Invalid PKCS12 password or authentication code".to_string())
        }};
    }
    match mac.mac.algorithm.oid.to_string().as_str() {
        "1.3.14.3.2.26" => digest!(sha1::Sha1),
        "2.16.840.1.101.3.4.2.1" => digest!(sha2_mac::Sha256),
        "2.16.840.1.101.3.4.2.2" => digest!(sha2_mac::Sha384),
        "2.16.840.1.101.3.4.2.3" => digest!(sha2_mac::Sha512),
        "2.16.840.1.101.3.4.2.4" => digest!(sha2_mac::Sha224),
        "2.16.840.1.101.3.4.2.5" => digest!(sha2_mac::Sha512_224),
        "2.16.840.1.101.3.4.2.6" => digest!(sha2_mac::Sha512_256),
        _ => Err("Unsupported PKCS12 authentication algorithm".into()),
    }
}

fn decrypt(
    oid: ObjectIdentifier,
    parameters: Option<&Any>,
    bytes: &[u8],
    password: &str,
) -> Result<Vec<u8>> {
    let parameters = parameters
        .ok_or("Missing PKCS12 encryption parameters")?
        .to_der()
        .map_err(error)?;
    if oid == PBES2 {
        let scheme = pkcs5::pbes2::Parameters::from_der(&parameters).map_err(error)?;
        match &scheme.kdf {
            pkcs5::pbes2::Kdf::Pbkdf2(params) => {
                if params.iteration_count == 0 || params.iteration_count > MAX_ITERATIONS as u32 {
                    return Err("PKCS12 iteration count out of range".into());
                }
            }
            pkcs5::pbes2::Kdf::Scrypt(params) => {
                let memory = params
                    .cost_parameter
                    .checked_mul(u64::from(params.block_size))
                    .and_then(|value| value.checked_mul(128));
                if params.parallelization > 16 || memory.is_none_or(|bytes| bytes > 1 << 30) {
                    return Err("PKCS12 scrypt resource limit exceeded".into());
                }
            }
            _ => return Err("Unsupported PKCS12 key derivation algorithm".into()),
        }
        return scheme.decrypt(password.as_bytes(), bytes).map_err(error);
    }
    if oid != pkcs12::PKCS_12_PBE_WITH_SHAAND3_KEY_TRIPLE_DES_CBC
        && oid != pkcs12::PKCS_12_PBEWITH_SHAAND40_BIT_RC2_CBC
    {
        return Err("Unsupported PKCS12 encryption algorithm".into());
    }
    let params = Pkcs12PbeParams::from_der(&parameters).map_err(error)?;
    iterations(params.iterations)?;
    let size = if oid == pkcs12::PKCS_12_PBE_WITH_SHAAND3_KEY_TRIPLE_DES_CBC {
        24
    } else {
        5
    };
    let key = derive_key_utf8::<sha1::Sha1>(
        password,
        params.salt.as_bytes(),
        Pkcs12KeyType::EncryptionKey,
        params.iterations,
        size,
    )
    .map_err(error)?;
    let iv = derive_key_utf8::<sha1::Sha1>(
        password,
        params.salt.as_bytes(),
        Pkcs12KeyType::Iv,
        params.iterations,
        8,
    )
    .map_err(error)?;
    use cbc::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
    if size == 24 {
        cbc::Decryptor::<des::TdesEde3>::new_from_slices(&key, &iv)
            .map_err(error)?
            .decrypt_padded_vec::<Pkcs7>(bytes)
            .map_err(error)
    } else {
        cbc::Decryptor::<rc2::Rc2>::new_from_slices(&key, &iv)
            .map_err(error)?
            .decrypt_padded_vec::<Pkcs7>(bytes)
            .map_err(error)
    }
}

fn safe_bytes(safe: &ContentInfo, password: &str) -> Result<Vec<u8>> {
    let content = safe.content.to_der().map_err(error)?;
    match safe.content_type {
        DATA => Ok(OctetString::from_der(&content)
            .map_err(error)?
            .into_bytes()
            .into_vec()),
        ENCRYPTED => {
            let encrypted = EncryptedData::from_der(&content).map_err(error)?;
            if encrypted.version != CmsVersion::V0 {
                return Err("Invalid PKCS12 encrypted content version".into());
            }
            let info = encrypted.enc_content_info;
            let bytes = info
                .encrypted_content
                .ok_or("Missing PKCS12 encrypted content")?;
            decrypt(
                info.content_enc_alg.oid,
                info.content_enc_alg.parameters.as_ref(),
                bytes.as_bytes(),
                password,
            )
        }
        _ => Err("Unsupported PKCS12 content type".into()),
    }
}

fn attribute(bag: &SafeBag, oid: ObjectIdentifier) -> Option<Vec<u8>> {
    bag.bag_attributes
        .as_ref()?
        .iter()
        .find(|a| a.oid == oid)?
        .values
        .iter()
        .next()?
        .to_der()
        .ok()
}

fn bags(
    bytes: &[u8],
    password: &str,
    keys: &mut Vec<Key>,
    certs: &mut Vec<Certificate>,
) -> Result<()> {
    for bag in Vec::<SafeBag>::from_der(bytes).map_err(error)? {
        let id = attribute(&bag, LOCAL_ID)
            .and_then(|bytes| OctetString::from_der(&bytes).ok())
            .map(|v| v.into_bytes().into_vec());
        let alias = attribute(&bag, NAME)
            .and_then(|bytes| BmpString::from_der(&bytes).ok())
            .map(|v| v.to_string());
        match bag.bag_id {
            pkcs12::PKCS_12_KEY_BAG_OID | pkcs12::PKCS_12_PKCS8_KEY_BAG_OID => {
                let der = if bag.bag_id == pkcs12::PKCS_12_KEY_BAG_OID {
                    ContextSpecific::<pkcs12::safe_bag::PrivateKeyInfo>::from_der(&bag.bag_value)
                        .map_err(error)?
                        .value
                        .to_der()
                        .map_err(error)?
                } else {
                    let encrypted =
                        ContextSpecific::<EncryptedPrivateKeyInfo>::from_der(&bag.bag_value)
                            .map_err(error)?
                            .value;
                    decrypt(
                        encrypted.encryption_algorithm.oid,
                        encrypted.encryption_algorithm.parameters.as_ref(),
                        encrypted.encrypted_data.as_bytes(),
                        password,
                    )?
                };
                if let Some(id) = id {
                    pkcs8::PrivateKeyInfoRef::try_from(der.as_slice()).map_err(error)?;
                    keys.push(Key { id, alias, der });
                }
            }
            pkcs12::PKCS_12_CERT_BAG_OID => {
                let cert = ContextSpecific::<CertBag>::from_der(&bag.bag_value)
                    .map_err(error)?
                    .value;
                if cert.cert_id != pkcs12::PKCS_12_X509_CERT_OID {
                    return Err("Unsupported PKCS12 certificate type".into());
                }
                let der = cert.cert_value.into_bytes().into_vec();
                let (_, certificate) = x509_parser::parse_x509_certificate(&der).map_err(error)?;
                let subject = certificate.subject().to_string();
                let issuer = certificate.issuer().to_string();
                let trusted = attribute(&bag, TRUST)
                    .and_then(|v| ObjectIdentifier::from_der(&v).ok())
                    == Some(ANY_USAGE);
                certs.push(Certificate {
                    id,
                    der,
                    subject,
                    issuer,
                    trusted,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn select(keys: Vec<Key>, certs: &[Certificate]) -> Result<(Vec<Vec<u8>>, Vec<u8>)> {
    let mut selected = std::collections::BTreeMap::new();
    for key in keys {
        if let Some(leaf) = certs.iter().find(|cert| cert.id.as_ref() == Some(&key.id)) {
            selected.insert(
                key.alias.clone().unwrap_or_else(|| leaf.subject.clone()),
                (key, leaf),
            );
        }
    }
    let (_, (key, leaf)) = selected
        .into_iter()
        .next()
        .ok_or("No private key found in certificate file")?;
    let mut chain = vec![leaf.der.clone()];
    let mut current = leaf;
    while let Some(issuer) = certs
        .iter()
        .find(|cert| !cert.trusted && cert.subject == current.issuer)
    {
        if chain.contains(&issuer.der) {
            break;
        }
        if issuer.subject != leaf.subject {
            chain.push(issuer.der.clone());
        }
        if issuer.subject == issuer.issuer {
            break;
        }
        current = issuer;
    }
    Ok((chain, key.der))
}

pub(super) fn decode(bytes: &[u8], password: &str) -> Result<(Vec<Vec<u8>>, Vec<u8>)> {
    let pfx = Pfx::from_der(bytes).map_err(error)?;
    if pfx.auth_safe.content_type != DATA {
        return Err("Unsupported PKCS12 authenticated content type".into());
    }
    let content = safe_bytes(&pfx.auth_safe, password)?;
    if let Some(mac) = pfx.mac_data.as_ref() {
        authenticate(mac, password, &content)?;
    }
    let mut keys = Vec::new();
    let mut certs = Vec::new();
    for safe in AuthenticatedSafe::from_der(&content).map_err(error)? {
        bags(
            &safe_bytes(&safe, password)?,
            password,
            &mut keys,
            &mut certs,
        )?;
    }
    select(keys, &certs)
}

#[cfg(test)]
#[path = "../../tests/unit/tls/pkcs12.rs"]
mod tests;
