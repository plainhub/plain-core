use super::*;

#[test]
fn short_uuid_uses_full_length_url_safe_ids() {
    let id = short_uuid();
    assert_eq!(id.len(), 22);
    assert!(id.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    assert!(!id.chars().any(|c| matches!(c, '0' | 'O' | '1' | 'I' | 'l')));
}
