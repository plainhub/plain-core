use super::*;

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "plain-rs-tls-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn generates_then_reloads() {
    let dir = tmp_dir("gen");
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let sans = vec!["localhost".to_string(), "127.0.0.1".to_string()];

    let (c1, k1) = ensure_self_signed_pem(&cert, &key, &sans).unwrap();
    assert!(c1.starts_with(b"-----BEGIN CERTIFICATE-----"));
    assert!(k1.starts_with(b"-----BEGIN PRIVATE KEY-----"));
    assert!(cert.exists() && key.exists());

    // Second call must load from disk, not regenerate.
    let (c2, k2) = ensure_self_signed_pem(&cert, &key, &sans).unwrap();
    assert_eq!(c1, c2);
    assert_eq!(k1, k2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn regenerates_when_one_file_missing() {
    let dir = tmp_dir("partial");
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let sans = vec!["plainapp.local".to_string()];
    let (c1, _) = ensure_self_signed_pem(&cert, &key, &sans).unwrap();
    std::fs::remove_file(&key).unwrap();
    let (c2, _) = ensure_self_signed_pem(&cert, &key, &sans).unwrap();
    // Fresh pair — cert differs from the previous generation.
    assert_ne!(c1, c2);
    assert!(key.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
