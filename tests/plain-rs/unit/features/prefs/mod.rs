//! Unit tests for `src/prefs/mod.rs` — compiled as the `tests` child
//! module via `#[cfg(test)] #[path]` there.
use super::*;

fn tmp_prefs(tag: &str) -> (std::path::PathBuf, Prefs) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "plain-rs-prefs-{tag}-{}-{nanos}",
        std::process::id()
    ));
    let path = dir.join("prefs.json");
    (dir.clone(), Prefs::load(&path).unwrap())
}

#[test]
fn missing_file_starts_empty_and_creates_on_first_set() {
    let (dir, prefs) = tmp_prefs("missing");
    assert_eq!(prefs.get::<String>("device_name").unwrap(), None);
    assert!(prefs.set("device_name", "box").unwrap());
    let file = dir.join("prefs.json");
    assert!(file.exists(), "file created on first set");
    assert!(
        !dir.join("prefs.json.tmp").exists(),
        "tmp file renamed away"
    );
}

#[test]
fn roundtrip_string_and_json_shapes() {
    let (_dir, prefs) = tmp_prefs("roundtrip");
    prefs.set("url_token", "abc123").unwrap();
    prefs.set("media_source_dirs", vec!["/a", "/b"]).unwrap();
    prefs
        .set("samba_settings", serde_json::json!({"enabled": true}))
        .unwrap();

    assert_eq!(
        prefs.get::<String>("url_token").unwrap().as_deref(),
        Some("abc123")
    );
    assert_eq!(
        prefs.get::<Vec<String>>("media_source_dirs").unwrap(),
        Some(vec!["/a".into(), "/b".into()])
    );
    assert_eq!(
        prefs.get::<serde_json::Value>("samba_settings").unwrap(),
        Some(serde_json::json!({"enabled": true}))
    );
}

#[test]
fn user_store_has_matching_default_and_clear_operations() {
    let (dir, prefs) = tmp_prefs("user-ops");
    assert_eq!(prefs.get_or("missing", 42), 42);
    prefs.set("developer_mode", true).unwrap();
    prefs.set("locale", "value").unwrap();
    prefs.clear().unwrap();
    assert!(prefs.entries().is_empty());
    assert!(!prefs.get_or("developer_mode", false));
    let reloaded = Prefs::load(&default_path(&dir)).unwrap();
    assert!(reloaded.entries().is_empty());
    assert!(!reloaded.get_or("developer_mode", false));
}

#[test]
fn set_same_value_skips_disk_write() {
    let (dir, prefs) = tmp_prefs("noop");
    assert!(prefs.set("volume_alias", 1u32).unwrap());
    let file = dir.join("prefs.json");
    let first = std::fs::read(&file).unwrap();
    // Same value again: no rewrite reported.
    assert!(!prefs.set("volume_alias", 1u32).unwrap());
    assert_eq!(std::fs::read(&file).unwrap(), first);
    // Different value: rewritten.
    assert!(prefs.set("volume_alias", 2u32).unwrap());
    assert_ne!(std::fs::read(&file).unwrap(), first);
}

#[test]
fn remove_persists_and_reports_presence() {
    let (_dir, prefs) = tmp_prefs("remove");
    prefs.set("volume_alias", "v").unwrap();
    assert!(prefs.remove("volume_alias").unwrap());
    assert!(
        !prefs.remove("volume_alias").unwrap(),
        "second remove: absent"
    );
    assert_eq!(prefs.get::<String>("volume_alias").unwrap(), None);
}

#[test]
fn clear_empties_and_persists() {
    let (dir, prefs) = tmp_prefs("clear");
    prefs.set("audio_sort_by", "DATE_DESC").unwrap();
    prefs.set("locale", "x").unwrap();
    prefs.clear().unwrap();
    assert_eq!(prefs.entries(), vec![]);
    let reloaded = Prefs::load(&dir.join("prefs.json")).unwrap();
    assert_eq!(reloaded.entries(), vec![]);
}

#[test]
fn get_or_returns_stored_or_default() {
    let (_dir, prefs) = tmp_prefs("getor");
    prefs.set("http_port", 9000u64).unwrap();
    assert_eq!(prefs.get_or("http_port", 8080u64), 9000);
    assert_eq!(prefs.get_or("https_port", 8443u64), 8443);
    // Type mismatch falls back to the default instead of erroring out.
    prefs.set("developer_mode", "not-a-bool").unwrap();
    assert!(!prefs.get_or("developer_mode", false));
}

#[test]
fn entries_sorted_with_json_rendered_values() {
    let (_dir, prefs) = tmp_prefs("entries");
    prefs.set("device_name", "v").unwrap();
    prefs.set("api_permissions", vec!["A", "B"]).unwrap();

    let entries = prefs.entries_sorted();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].0, "api_permissions");
    assert_eq!(entries[0].1, "[\"A\",\"B\"]");
    assert_eq!(entries[1].0, "device_name");
    // Strings render as JSON (quoted) — plain-desktop parity.
    assert_eq!(entries[1].1, "\"v\"");

    let values = prefs.entries();
    assert_eq!(values[0].1, serde_json::json!(["A", "B"]));
    assert_eq!(values[1].1, serde_json::json!("v"));
}

#[test]
fn reload_sees_previous_writes_and_pretty_prints() {
    let (dir, prefs) = tmp_prefs("reload");
    prefs.set("device_name", "box").unwrap();
    prefs
        .set("samba_settings", serde_json::json!({"a": 1}))
        .unwrap();
    drop(prefs);

    // File is pretty-printed (hand-editable, like plain-desktop's).
    let text = std::fs::read_to_string(dir.join("prefs.json")).unwrap();
    assert!(text.contains("\n  \"device_name\":"), "pretty: {text}");

    let reloaded = Prefs::load(&dir.join("prefs.json")).unwrap();
    assert_eq!(
        reloaded.get::<String>("device_name").unwrap().as_deref(),
        Some("box")
    );
    assert_eq!(
        reloaded.get::<serde_json::Value>("samba_settings").unwrap(),
        Some(serde_json::json!({"a": 1}))
    );
}

/// Both preference files load independently and retain their own key spaces.

#[test]
fn corrupt_file_fails_loudly() {
    let (dir, _prefs) = tmp_prefs("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("prefs.json"), "{not json").unwrap();
    assert!(Prefs::load(&dir.join("prefs.json")).is_err());
}

#[test]
fn global_accessor_contract() {
    let (_dir, prefs) = tmp_prefs("global");
    let prefs = std::sync::Arc::new(prefs);
    prefs.set("device_name", "global-box").unwrap();
    set_global(prefs);
    assert_eq!(
        get_default()
            .get::<String>("device_name")
            .unwrap()
            .as_deref(),
        Some("global-box")
    );
    assert!(try_get_default().is_some());
}
