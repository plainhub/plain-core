// Certificate profile adapted from rcgen 0.13.2 (https://github.com/rustls/rcgen), see LICENSE.

use std::{io, net::IpAddr, sync::LazyLock};

use der::{
    DateTime, Decode, Encode, Sequence,
    asn1::{AnyRef, BitStringRef, Ia5String, ObjectIdentifier, OctetString, UintRef},
};
use ring::{
    digest,
    pkcs8::Document,
    rand::SystemRandom,
    signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair},
};
use x509_cert::{
    ext::{
        Extension, Extensions,
        pkix::{SubjectAltName, name::GeneralName},
    },
    name::Name,
    spki::{AlgorithmIdentifier, AlgorithmIdentifierRef, SubjectPublicKeyInfo},
    time::Validity,
};

const ECDSA_SHA256: AlgorithmIdentifierRef<'static> = AlgorithmIdentifierRef {
    oid: ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2"),
    parameters: None,
};
const EC_PUBLIC_KEY: AlgorithmIdentifier<ObjectIdentifier> = AlgorithmIdentifier {
    oid: ObjectIdentifier::new_unwrap("1.2.840.10045.2.1"),
    parameters: Some(ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7")),
};
static NAME: LazyLock<Vec<u8>> = LazyLock::new(|| {
    "CN=rcgen self signed cert"
        .parse::<Name>()
        .expect("valid constant certificate name")
        .to_der()
        .expect("encodable constant certificate name")
});
static VALIDITY: LazyLock<Validity> = LazyLock::new(|| {
    Validity::new(
        DateTime::new(1975, 1, 1, 0, 0, 0)
            .expect("valid certificate start date")
            .into(),
        DateTime::new(4096, 1, 1, 0, 0, 0)
            .expect("valid certificate end date")
            .into(),
    )
});

#[derive(Sequence)]
struct CertificateBody<'a> {
    #[asn1(context_specific = "0")]
    version: u8,
    serial: UintRef<'a>,
    signature: AlgorithmIdentifierRef<'a>,
    issuer: AnyRef<'a>,
    validity: Validity,
    subject: AnyRef<'a>,
    public_key: SubjectPublicKeyInfo<ObjectIdentifier, BitStringRef<'a>>,
    #[asn1(context_specific = "3", optional = "true")]
    extensions: Option<Extensions>,
}

#[derive(Sequence)]
struct Certificate<'a> {
    body: AnyRef<'a>,
    signature_algorithm: AlgorithmIdentifierRef<'a>,
    signature: BitStringRef<'a>,
}

pub(super) fn generate(names: &[String]) -> io::Result<(Vec<u8>, Document)> {
    let random = SystemRandom::new();
    let key_der = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &random)
        .map_err(|_| io::Error::other("TLS key generation failed"))?;
    let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, key_der.as_ref(), &random)
        .map_err(|_| io::Error::other("TLS key import failed"))?;
    let body = certificate_body(key.public_key().as_ref(), names).map_err(io::Error::other)?;
    let signature = key
        .sign(&random, &body)
        .map_err(|_| io::Error::other("TLS certificate signing failed"))?;
    let cert = Certificate {
        body: AnyRef::from_der(&body).map_err(io::Error::other)?,
        signature_algorithm: ECDSA_SHA256,
        signature: BitStringRef::from_bytes(signature.as_ref()).map_err(io::Error::other)?,
    };
    Ok((cert.to_der().map_err(io::Error::other)?, key_der))
}

fn certificate_body(public_key: &[u8], names: &[String]) -> der::Result<Vec<u8>> {
    let hash = digest::digest(&digest::SHA256, public_key);
    let mut serial = [0; 20];
    serial.copy_from_slice(&hash.as_ref()[..20]);
    serial[0] &= 0x7f;

    let name = AnyRef::from_der(&NAME)?;
    let extensions = if names.is_empty() {
        None
    } else {
        let names = names
            .iter()
            .map(|name| match name.parse::<IpAddr>() {
                Ok(address) => Ok(address.into()),
                Err(_) => Ok(GeneralName::DnsName(Ia5String::new(name)?)),
            })
            .collect::<der::Result<Vec<_>>>()?;
        Some(vec![Extension {
            extn_id: ObjectIdentifier::new_unwrap("2.5.29.17"),
            critical: false,
            extn_value: OctetString::new(SubjectAltName(names).to_der()?)?,
        }])
    };
    CertificateBody {
        version: 2,
        serial: UintRef::new(&serial)?,
        signature: ECDSA_SHA256,
        issuer: name,
        validity: *VALIDITY,
        subject: name,
        public_key: SubjectPublicKeyInfo {
            algorithm: EC_PUBLIC_KEY,
            subject_public_key: BitStringRef::from_bytes(public_key)?,
        },
        extensions,
    }
    .to_der()
}
