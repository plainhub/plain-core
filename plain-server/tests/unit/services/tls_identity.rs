use super::*;

#[test]
fn config_preserves_chains_mixed_sections_line_endings_and_alpn() {
    let first = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let second = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = first.cert.pem();
    let key = first.key_pair.serialize_pem();
    let chain = format!("text before PEM\n{key}{certificate}{}", second.cert.pem());
    let mixed_key = format!("{certificate}{key}");
    let configured = config(chain.as_bytes(), mixed_key.as_bytes()).unwrap();
    assert_eq!(
        configured.alpn_protocols,
        [b"h2".to_vec(), b"http/1.1".to_vec()]
    );
    assert_eq!(
        signature(chain.as_bytes()).unwrap(),
        signature(certificate.as_bytes()).unwrap()
    );
    assert!(
        config(
            certificate.replace('\n', "\r\n").as_bytes(),
            key.replace('\n', "\r\n").as_bytes()
        )
        .is_ok()
    );
    assert!(config(certificate.trim_end().as_bytes(), key.trim_end().as_bytes()).is_ok());
    assert!(
        config(
            certificate.as_bytes(),
            second.key_pair.serialize_pem().as_bytes()
        )
        .is_err()
    );
}

#[test]
fn pem_failures_are_rejected_without_skipping_a_malformed_recognized_section() {
    let identity = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = identity.cert.pem();
    let key = identity.key_pair.serialize_pem();
    for malformed in [
        "-----BEGIN CERTIFICATE-----\n@@@\n-----END CERTIFICATE-----\n",
        "-----BEGIN CERTIFICATE-----\nAQID\n",
        "-----BEGIN CERTIFICATE-----\nAQID\n-----END PRIVATE KEY-----\n",
    ] {
        assert!(signature(malformed.as_bytes()).is_err());
        assert!(config(malformed.as_bytes(), key.as_bytes()).is_err());
    }
    let bad_key = "-----BEGIN PRIVATE KEY-----\n@@@\n-----END PRIVATE KEY-----\n";
    assert!(config(certificate.as_bytes(), format!("{bad_key}{key}").as_bytes()).is_err());
    assert!(config(format!("{certificate}{bad_key}").as_bytes(), key.as_bytes()).is_err());
    assert!(config(b"", key.as_bytes()).is_err());
    assert_eq!(
        config(certificate.as_bytes(), b"no key").err().as_deref(),
        Some("TLS private key is missing")
    );
}

#[test]
fn private_key_reader_preserves_all_three_pem_encodings_and_first_key_selection() {
    for (label, kind) in [
        ("RSA PRIVATE KEY", "pkcs1"),
        ("EC PRIVATE KEY", "sec1"),
        ("PRIVATE KEY", "pkcs8"),
    ] {
        let pem = format!("-----BEGIN {label}-----\nAQID\n-----END {label}-----\n");
        let key = PrivateKeyDer::pem_slice_iter(pem.as_bytes())
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(key.secret_der(), [1, 2, 3]);
        assert_eq!(
            match key {
                PrivateKeyDer::Pkcs1(_) => "pkcs1",
                PrivateKeyDer::Sec1(_) => "sec1",
                PrivateKeyDer::Pkcs8(_) => "pkcs8",
                _ => unreachable!(),
            },
            kind
        );
    }
    let identity = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = identity.cert.pem();
    let key = identity.key_pair.serialize_pem();
    let following = "-----BEGIN PRIVATE KEY-----\n@@@\n-----END PRIVATE KEY-----\n";
    assert!(
        config(
            certificate.as_bytes(),
            format!("{key}{following}").as_bytes()
        )
        .is_ok()
    );
}
#[test]
fn persists_rotates_and_rejects_invalid_identity() {
    let dir = std::env::temp_dir().join(format!("plain-mobile-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("identity.json");
    let first = load(&path).unwrap();
    assert_eq!(first, load(&path).unwrap());
    assert!(!signature(&first.0).unwrap().is_empty());
    let second = generate(&path).unwrap();
    assert_ne!(signature(&first.0).unwrap(), signature(&second.0).unwrap());
    assert!(save(&path, &first.0, &second.1).is_err());
    assert_eq!(second, load(&path).unwrap());
    assert!(signature(b"invalid PEM").is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn rejects_truncated_and_overflowing_der() {
    for bytes in [
        &[0x30, 0x80][..],
        &[0x30, 0x82, 0xff][..],
        &[0x30, 0x01][..],
        &[0x30, 0x89, 0, 0, 0, 0, 0, 0, 0, 0, 0][..],
    ] {
        let mut input = bytes;
        assert!(tlv(&mut input, 0x30).is_err());
    }
}

#[test]
fn configured_pem_identity_is_shared_by_restart_and_certificate_rotation() {
    let directory = tempfile::tempdir().unwrap();
    let certificate = directory.path().join("cert.pem");
    let key = directory.path().join("key.pem");
    let source = IdentitySource::Pem {
        certificate: certificate.clone(),
        key: key.clone(),
    };
    let first = source.identity().unwrap();
    assert_eq!(source.identity().unwrap(), first);
    source.action(r#"{"action":"generate"}"#).unwrap();
    let rotated = source.identity().unwrap();
    assert_ne!(rotated.0, first.0);
    assert_eq!(rotated.0, std::fs::read(certificate).unwrap());
    assert_eq!(rotated.1, std::fs::read(key).unwrap());
    assert!(!directory.path().join("tls-identity.json").exists());
}

#[test]
fn pkcs12_import_persists_a_matching_identity_and_failed_import_preserves_it() {
    let directory = tempfile::tempdir().unwrap();
    let sources = [
        IdentitySource::Json(directory.path().join("identity.json")),
        IdentitySource::Pem { certificate: directory.path().join("cert.pem"), key: directory.path().join("key.pem") },
    ];
    let bytes = include_bytes!("../../../../plain-rs/testdata/crypto/pkcs12/aes-sha256.p12");
    let data = plain_rs::utils::base64::base64_encode(bytes);
    let expected = plain_rs::tls::decode_pkcs12(bytes, "password").unwrap();
    for source in sources {
        source.identity().unwrap();
        let result = source.action(&json!({"action": "importPkcs12", "data": data, "password": "password"}).to_string()).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&result).unwrap()["signature"], json!(signature(&expected.0).unwrap()));
        assert_eq!(source.identity().unwrap(), expected);
        config(&expected.0, &expected.1).unwrap();
        for (data, password) in [(data.as_str(), "wrong"), ("invalid", "password")] {
            assert!(source.action(&json!({"action": "importPkcs12", "data": data, "password": password}).to_string()).is_err());
            assert_eq!(source.identity().unwrap(), expected);
        }
    }
}

#[test]
fn certificate_fingerprint_hashes_the_leaf_der() {
    use sha2::{Digest, Sha256};
    let leaf = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let other = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let expected = plain_rs::utils::hex::bytes_to_hex(&Sha256::digest(leaf.cert.der().as_ref()));
    assert_eq!(certificate_sha256(leaf.cert.pem().as_bytes()).unwrap(), expected);
    let chain = format!("{}{}", leaf.cert.pem().replace('\n', "\r\n"), other.cert.pem());
    assert_eq!(certificate_sha256(chain.as_bytes()).unwrap(), expected);
    assert_ne!(certificate_sha256(other.cert.pem().as_bytes()).unwrap(), expected);
    assert!(certificate_sha256(b"invalid certificate").is_err());
}
