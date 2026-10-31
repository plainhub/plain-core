use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/crypto/pkcs12")
            .join(name),
    )
    .unwrap()
}

#[test]
fn fixed_imports_preserve_key_and_certificate_across_encryption_and_mac_algorithms() {
    for cipher in ["aes", "rc2", "des"] {
        for mac in [
            "sha1",
            "sha224",
            "sha256",
            "sha384",
            "sha512",
            "sha512-224",
            "sha512-256",
        ] {
            let bytes = fixture(&format!("{cipher}-{mac}.p12"));
            let (chain, key) = decode(&bytes, "password").unwrap();
            assert_eq!(chain, [fixture("certificate.der")]);
            assert_eq!(key, fixture("private-key.der"));
            assert!(decode(&bytes, "wrong").is_err());
        }
    }
    for (name, password) in [
        ("empty-password.p12", ""),
        ("unicode-password.p12", "密码é"),
    ] {
        let (chain, key) = decode(&fixture(name), password).unwrap();
        assert_eq!(chain, [fixture("certificate.der")]);
        assert_eq!(key, fixture("private-key.der"));
    }
    assert!(decode(&fixture("empty-store.p12"), "").is_err());
}

#[test]
fn corruption_truncation_and_unsupported_content_are_rejected() {
    let bytes = fixture("aes-sha256.p12");
    for length in 0..bytes.len() {
        assert!(
            decode(&bytes[..length], "password").is_err(),
            "length {length}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing, "password").is_err());
    let mut pfx = Pfx::from_der(&bytes).unwrap();
    let mac = pfx.mac_data.as_mut().unwrap();
    mac.mac.digest = OctetString::new(vec![0; 32]).unwrap();
    assert!(
        decode(&pfx.to_der().unwrap(), "password")
            .unwrap_err()
            .contains("authentication")
    );
    pfx.auth_safe.content_type = ENCRYPTED;
    assert!(
        decode(&pfx.to_der().unwrap(), "password")
            .unwrap_err()
            .contains("content type")
    );
}

#[test]
fn authentication_limits_and_unsupported_algorithms_fail_before_derivation() {
    let pfx = Pfx::from_der(&fixture("aes-sha256.p12")).unwrap();
    let mut mac = pfx.mac_data.unwrap();
    for count in [i32::MIN, -1, 0, MAX_ITERATIONS + 1] {
        mac.iterations = count;
        assert!(
            authenticate(&mac, "password", b"content")
                .unwrap_err()
                .contains("iteration")
        );
    }
    assert!(iterations(1).is_ok());
    assert!(iterations(MAX_ITERATIONS).is_ok());
    mac.iterations = 1;
    mac.mac.algorithm.oid = DATA;
    assert!(
        authenticate(&mac, "password", b"content")
            .unwrap_err()
            .contains("Unsupported")
    );
}

fn plaintext_bags() -> Vec<SafeBag> {
    let pfx = Pfx::from_der(&fixture("aes-sha256.p12")).unwrap();
    let content = safe_bytes(&pfx.auth_safe, "password").unwrap();
    AuthenticatedSafe::from_der(&content)
        .unwrap()
        .iter()
        .flat_map(|safe| Vec::<SafeBag>::from_der(&safe_bytes(safe, "password").unwrap()).unwrap())
        .collect()
}

fn unencrypted_pfx(mut bags: Vec<SafeBag>) -> Vec<u8> {
    for bag in &mut bags {
        bag.bag_value = Any::from_der(&bag.bag_value).unwrap().value().to_vec();
    }
    let safe = ContentInfo {
        content_type: DATA,
        content: Any::encode_from(&OctetString::new(bags.to_der().unwrap()).unwrap()).unwrap(),
    };
    let content =
        Any::encode_from(&OctetString::new(vec![safe].to_der().unwrap()).unwrap()).unwrap();
    Pfx {
        version: pkcs12::pfx::Version::V3,
        auth_safe: ContentInfo {
            content_type: DATA,
            content,
        },
        mac_data: None,
    }
    .to_der()
    .unwrap()
}

#[test]
fn optional_mac_and_bag_order_preserve_strict_key_linking() {
    let mut values = plaintext_bags();
    values.reverse();
    assert_eq!(
        decode(&unencrypted_pfx(values.clone()), "password")
            .unwrap()
            .1,
        fixture("private-key.der")
    );
    for bag in &mut values {
        if bag.bag_id == pkcs12::PKCS_12_PKCS8_KEY_BAG_OID {
            bag.bag_attributes = None;
        }
    }
    assert!(decode(&unencrypted_pfx(values), "password").is_err());
    let mut values = plaintext_bags();
    for bag in &mut values {
        if bag.bag_id == pkcs12::PKCS_12_CERT_BAG_OID {
            bag.bag_attributes = None;
        }
    }
    assert!(decode(&unencrypted_pfx(values), "password").is_err());
}

#[test]
fn malformed_recognized_bags_and_unsupported_certificates_are_rejected() {
    for kind in [
        pkcs12::PKCS_12_KEY_BAG_OID,
        pkcs12::PKCS_12_PKCS8_KEY_BAG_OID,
        pkcs12::PKCS_12_CERT_BAG_OID,
    ] {
        let mut values = plaintext_bags();
        values[0].bag_id = kind;
        values[0].bag_value = vec![0xa0, 2, 5, 0];
        assert!(decode(&unencrypted_pfx(values), "password").is_err());
    }
    let mut values = plaintext_bags();
    let bag = values
        .iter_mut()
        .find(|v| v.bag_id == pkcs12::PKCS_12_CERT_BAG_OID)
        .unwrap();
    let mut cert = ContextSpecific::<CertBag>::from_der(&bag.bag_value).unwrap();
    cert.value.cert_id = DATA;
    bag.bag_value = cert.to_der().unwrap();
    assert!(
        decode(&unencrypted_pfx(values), "password")
            .unwrap_err()
            .contains("certificate type")
    );
}

fn certificate(
    id: Option<u8>,
    data: u8,
    subject: &str,
    issuer: &str,
    trusted: bool,
) -> Certificate {
    Certificate {
        id: id.map(|v| vec![v]),
        der: vec![data],
        subject: subject.into(),
        issuer: issuer.into(),
        trusted,
    }
}
fn key(id: u8, alias: Option<&str>) -> Key {
    Key {
        id: vec![id],
        der: vec![id],
        alias: alias.map(str::to_string),
    }
}

#[test]
fn chain_selection_preserves_alias_order_linking_trust_and_cycle_bounds() {
    let certs = vec![
        certificate(Some(1), 11, "leaf", "middle", false),
        certificate(None, 12, "middle", "root", false),
        certificate(None, 13, "root", "root", false),
        certificate(Some(2), 14, "other", "other", false),
    ];
    let (chain, selected) = select(vec![key(1, Some("a")), key(2, Some("z"))], &certs).unwrap();
    assert_eq!(selected, [1]);
    assert_eq!(chain, [vec![11], vec![12], vec![13]]);
    assert_eq!(
        select(vec![key(1, None), key(2, None)], &certs).unwrap().1,
        [1]
    );
    assert!(select(vec![key(9, None)], &certs).is_err());
    let trusted = vec![
        certificate(Some(1), 11, "leaf", "root", false),
        certificate(None, 12, "root", "root", true),
    ];
    assert_eq!(select(vec![key(1, None)], &trusted).unwrap().0, [vec![11]]);
    let cycle = vec![
        certificate(Some(1), 11, "leaf", "middle", false),
        certificate(None, 12, "middle", "leaf", false),
    ];
    assert_eq!(
        select(vec![key(1, None)], &cycle).unwrap().0,
        [vec![11], vec![12]]
    );
}

#[test]
fn pbe_limits_and_bad_padding_are_rejected() {
    let bags = plaintext_bags();
    let bag = bags
        .iter()
        .find(|b| b.bag_id == pkcs12::PKCS_12_PKCS8_KEY_BAG_OID)
        .unwrap();
    let encrypted = ContextSpecific::<EncryptedPrivateKeyInfo>::from_der(&bag.bag_value)
        .unwrap()
        .value;
    let algorithm = encrypted.encryption_algorithm;
    assert!(decrypt(algorithm.oid, None, b"", "password").is_err());
    assert!(decrypt(DATA, algorithm.parameters.as_ref(), b"", "password").is_err());
    assert!(
        decrypt(
            algorithm.oid,
            algorithm.parameters.as_ref(),
            &[0; 16],
            "password"
        )
        .is_err()
    );
    let mut params =
        pkcs5::pbes2::Parameters::from_der(&algorithm.parameters.unwrap().to_der().unwrap())
            .unwrap();
    if let pkcs5::pbes2::Kdf::Pbkdf2(ref mut kdf) = params.kdf {
        kdf.iteration_count = MAX_ITERATIONS as u32 + 1;
    }
    let parameters = Any::encode_from(&params).unwrap();
    assert!(
        decrypt(PBES2, Some(&parameters), b"", "password")
            .unwrap_err()
            .contains("iteration")
    );
    let params = Pkcs12PbeParams {
        salt: OctetString::new(vec![0; 8]).unwrap(),
        iterations: MAX_ITERATIONS + 1,
    };
    let parameters = Any::encode_from(&params).unwrap();
    assert!(
        decrypt(
            pkcs12::PKCS_12_PBE_WITH_SHAAND3_KEY_TRIPLE_DES_CBC,
            Some(&parameters),
            b"",
            "password"
        )
        .unwrap_err()
        .contains("iteration")
    );
}

#[test]
fn independently_generated_plain_and_aes_certificate_chains_preserve_order() {
    for name in ["chain-aes128.p12", "chain-aes192.p12", "chain-plain.p12"] {
        let (chain, key) = decode(&fixture(name), "password").unwrap();
        assert_eq!(key, fixture("chain-key.der"));
        assert_eq!(
            chain,
            [
                fixture("chain-leaf.der"),
                fixture("chain-middle.der"),
                fixture("chain-root.der")
            ]
        );
    }
}

#[test]
fn scrypt_decrypts_and_rejects_excessive_cost_before_allocation() {
    use pkcs5::pbes2::{EncryptionScheme, Kdf, Parameters, ScryptParams};
    let mut params = Parameters {
        kdf: Kdf::Scrypt(ScryptParams {
            salt: b"fixture salt".as_slice().try_into().unwrap(),
            cost_parameter: 16,
            block_size: 1,
            parallelization: 1,
            key_length: None,
        }),
        encryption: EncryptionScheme::Aes128Cbc { iv: [0; 16] },
    };
    let ciphertext = params.encrypt("password", b"private bytes").unwrap();
    assert_eq!(
        decrypt(
            PBES2,
            Some(&Any::encode_from(&params).unwrap()),
            &ciphertext,
            "password"
        )
        .unwrap(),
        b"private bytes"
    );
    for (n, r, p) in [(1 << 30, 8, 1), (u64::MAX, u16::MAX, 1), (16, 1, 17)] {
        if let Kdf::Scrypt(ref mut kdf) = params.kdf {
            kdf.cost_parameter = n;
            kdf.block_size = r;
            kdf.parallelization = p;
        }
        let encoded = Any::encode_from(&params).unwrap();
        assert!(
            decrypt(PBES2, Some(&encoded), b"", "password")
                .unwrap_err()
                .contains("resource limit")
        );
    }
}
