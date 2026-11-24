//! Unit tests for `src/prefs/identity.rs` — compiled as the `tests`
//! child module via `#[cfg(test)] #[path]` there.
use super::*;

fn tmp_prefs(tag: &str) -> Prefs {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "plain-rs-prefs-id-{tag}-{}-{nanos}",
        std::process::id()
    ));
    Prefs::load(&dir.join("prefs.json")).unwrap()
}

#[test]
fn ensure_identity_persists_the_signing_seed() {
    let prefs = tmp_prefs("identity");
    let id = ensure_identity(&prefs);
    let uuid = plain_libs::utils::uuid::Uuid::parse_str(&id.client_id).expect("clientId must be a UUID");
    assert_eq!(&uuid.as_bytes()[..4], b"plai");
    assert_eq!(uuid.as_bytes()[6] >> 4, 8);
    assert_eq!(uuid.as_bytes()[8] & 0xc0, 0x80);
    assert!(!id.device_name.is_empty());
    assert!(!id.ed25519_keypair.is_empty());

    assert_eq!(
        prefs.get::<String>("client_id").unwrap().as_deref(),
        Some(id.client_id.as_str())
    );
    assert_eq!(
        prefs.get::<String>("device_name").unwrap().as_deref(),
        Some(id.device_name.as_str())
    );
    let decode = plain_libs::utils::base64::base64_decode;
    let seed = prefs.get::<String>("signature_private_key").unwrap().unwrap();
    assert_eq!(decode(&seed), decode(&id.ed25519_keypair)[..32]);
}

#[test]
fn bootstrap_derives_the_same_public_keys_after_reload() {
    use crate::features::prefs::{pref_keys::*, security};

    let prefs = tmp_prefs("public-keys");
    let old_pair = base64_encode(&ed25519_generate().0);
    let old = plain_libs::serde_json::json!({"signature_key_pair": old_pair});
    std::fs::create_dir_all(prefs.path().parent().unwrap()).unwrap();
    std::fs::write(prefs.path(), old.to_string()).unwrap();
    let prefs = Prefs::load(prefs.path()).unwrap();
    security::bootstrap(&prefs).unwrap();
    let signature = security::get_entry(&prefs, SIGNATURE_PUBLIC_KEY).unwrap();
    let encryption = security::encryption_public_key(&prefs).unwrap();
    assert_ne!(base64_encode(&security::signing_keypair(&prefs).unwrap()), old_pair);
    assert!(!prefs.entries().iter().any(|(key, _)| key == "signature_key_pair"));
    for key in [SIGNATURE_PRIVATE_KEY, ENCRYPTION_PRIVATE_KEY] {
        let encoded = prefs.get::<String>(key).unwrap().unwrap();
        assert_eq!(plain_libs::utils::base64::base64_decode(&encoded).len(), 32);
        assert!(security::get_entry(&prefs, key).unwrap().is_none());
        assert!(security::set_entry(&prefs, key, plain_libs::serde_json::json!("invalid")).is_err());
    }
    let reloaded = Prefs::load(prefs.path()).unwrap();
    security::bootstrap(&reloaded).unwrap();
    assert_eq!(security::get_entry(&reloaded, SIGNATURE_PUBLIC_KEY).unwrap(), signature);
    assert_eq!(security::encryption_public_key(&reloaded).unwrap(), encryption);
    assert!(!reloaded.entries().iter().any(|(key, _)| key.ends_with("public_key")));
}

#[test]
fn ensure_identity_is_idempotent_and_reuses_stored_values() {
    let prefs = tmp_prefs("idempotent");
    let first = ensure_identity(&prefs);
    let second = ensure_identity(&prefs);
    assert_eq!(first.client_id, second.client_id);
    assert_eq!(first.device_name, second.device_name);
    assert_eq!(first.ed25519_keypair, second.ed25519_keypair);

    // Pre-stored values win over generation.
    prefs.set("client_id", "706c6169-6a3b-8a0e-b4e8-26c3ea7efa78").unwrap();
    assert_eq!(ensure_identity(&prefs).client_id, "706c6169-6a3b-8a0e-b4e8-26c3ea7efa78");
}

#[test]
fn ensure_identity_normalizes_stored_device_name() {
    let prefs = tmp_prefs("normalize");
    prefs.set("device_name", "Box.local").unwrap();
    assert_eq!(ensure_identity(&prefs).device_name, "Box");
    prefs.set("device_name", "  ").unwrap();
    let regenerated = ensure_identity(&prefs).device_name;
    assert!(!regenerated.is_empty(), "blank name regenerates");
}

#[test]
fn ensure_mdns_hostname_generates_and_persists_the_pre_migration_format() {
    let prefs = tmp_prefs("mdns");
    let h = ensure_mdns_hostname(&prefs);
    assert!(h.ends_with(".local"));
    let label = h.strip_suffix(".local").unwrap();
    assert_eq!(label.len(), 2);
    assert!(
        label
            .bytes()
            .all(|byte| b"abcdefghjkmnpqrstuwxyz".contains(&byte))
    );
    assert_eq!(
        prefs.get::<String>("mdns_hostname").unwrap().as_deref(),
        Some(h.as_str())
    );
    let reloaded = Prefs::load(prefs.path()).unwrap();
    assert_eq!(ensure_mdns_hostname(&reloaded), h);
}

#[test]
fn default_device_name_prefers_hostname() {
    // Deterministic on the host running the test: either the trimmed
    // OS hostname or the Plain-<UUID> fallback — never empty, never
    // carrying a `.local` suffix.
    let name = default_device_name();
    assert!(!name.is_empty());
    assert!(!name.ends_with(".local"));
    assert!(!name.ends_with('.'));
}

#[test]
fn ensure_mdns_hostname_preserves_a_saved_custom_name() {
    let prefs = tmp_prefs("mdns-custom");
    prefs.set("mdns_hostname", "custom-device.local").unwrap();
    assert_eq!(ensure_mdns_hostname(&prefs), "custom-device.local");
    let reloaded = Prefs::load(prefs.path()).unwrap();
    assert_eq!(ensure_mdns_hostname(&reloaded), "custom-device.local");
}
