use super::*;
use plain_libs::crypto::{ed25519_keypair_from_seed, ed25519_sign};
use plain_libs::utils::base64::{base64_decode, base64_encode};

fn signed_manifest() -> (String, Vec<u8>, String) {
    let pair = ed25519_keypair_from_seed(&[42; 32]);
    let mut der = b"\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00".to_vec();
    der.extend_from_slice(&pair[32..]);
    let manifest = format!(
        "version 4.0.2\nschema {}\n{}  plainapp-linux-amd64.tar.gz\n{}  plainapp-linux-arm64.tar.gz\n{}  install.sh\n",
        "1".repeat(64),
        "a".repeat(64),
        "b".repeat(64),
        "c".repeat(64),
    );
    let signature = base64_decode(&ed25519_sign(&pair, manifest.as_bytes()));
    (manifest, signature, base64_encode(&der))
}

#[test]
fn verifies_signed_release_and_selects_the_requested_archive() {
    let (manifest, signature, key) = signed_manifest();
    for (arch, hash) in [("amd64", "a"), ("arm64", "b")] {
        assert_eq!(
            archive_hash(
                manifest.as_bytes(),
                &signature,
                &key,
                "4.0.2",
                &"1".repeat(64),
                &format!("plainapp-linux-{arch}.tar.gz")
            )
            .unwrap(),
            hash.repeat(64)
        );
    }
}

#[test]
fn rejects_modified_bytes_wrong_release_and_schema_changes() {
    let (manifest, signature, key) = signed_manifest();
    for (body, release, schema) in [
        (manifest.replace("4.0.2", "4.0.3"), "4.0.3", "1".repeat(64)),
        (manifest.clone(), "4.0.1", "1".repeat(64)),
        (manifest, "4.0.2", "2".repeat(64)),
    ] {
        assert!(
            archive_hash(
                body.as_bytes(),
                &signature,
                &key,
                release,
                &schema,
                "plainapp-linux-amd64.tar.gz"
            )
            .is_err()
        );
    }
}

#[test]
fn rejects_wrong_signing_key_and_modified_signature() {
    let (manifest, mut signature, key) = signed_manifest();
    let mut wrong_key = base64_decode(&key);
    wrong_key[12..].copy_from_slice(&ed25519_keypair_from_seed(&[43; 32])[32..]);
    assert!(
        archive_hash(
            manifest.as_bytes(),
            &signature,
            &base64_encode(&wrong_key),
            "4.0.2",
            &"1".repeat(64),
            "plainapp-linux-amd64.tar.gz"
        )
        .is_err()
    );
    signature[0] ^= 1;
    assert!(
        archive_hash(
            manifest.as_bytes(),
            &signature,
            &key,
            "4.0.2",
            &"1".repeat(64),
            "plainapp-linux-amd64.tar.gz"
        )
        .is_err()
    );
}

#[test]
fn compares_stable_versions_numerically_and_rejects_other_tags() {
    assert!(version("4.0.10").unwrap() > version("4.0.2").unwrap());
    for invalid in [
        "4.00.2",
        "4.0.2-beta",
        "4..2",
        "4.0",
        "4.0.2/other",
        "+4.0.2",
        "4.0.2.3",
    ] {
        assert!(version(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn shared_release_key_does_not_allow_package_swaps_between_products() {
    use plain_libs::crypto::release::package_hash;

    let pair = ed25519_keypair_from_seed(&[42; 32]);
    let (_, _, key) = signed_manifest();
    let app = "plainhub/plain-app";
    let desktop = "plainhub/plain-desktop";
    let package = "PlainApp-4.0.0-64bit-Recommended.apk";
    let digest = "1".repeat(64);
    let entry = format!("{digest}  {package}\n");
    let manifest = format!("version 4.0.0\nrepository {app}\n{entry}");
    let sign = |value: &str| base64_decode(&ed25519_sign(&pair, value.as_bytes()));
    let check = |product: &str, version: &str, asset: &str| {
        package_hash(
            manifest.as_bytes(),
            &sign(&manifest),
            &key,
            product,
            version,
            asset,
        )
    };
    assert_eq!(check(app, "4.0.0", package).unwrap(), digest);
    assert!(check(desktop, "4.0.0", package).is_err());
    assert!(check(app, "4.0.1", package).is_err());
    assert!(check(app, "4.0.0", "PlainApp-4.0.0-Old-32bit.apk").is_err());

    let duplicate = format!("{manifest}{entry}");
    assert!(
        package_hash(
            duplicate.as_bytes(),
            &sign(&duplicate),
            &key,
            app,
            "4.0.0",
            package
        )
        .is_err()
    );
}
