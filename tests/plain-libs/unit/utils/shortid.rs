use super::*;

#[test]
fn length_is_22() {
    assert_eq!(new_id().len(), 22);
}

#[test]
fn unique() {
    let a = new_id();
    let b = new_id();
    assert_ne!(a, b);
}

#[test]
fn no_ambiguous_chars() {
    for _ in 0..50 {
        let id = new_id();
        for c in id.chars() {
            assert!(
                !matches!(c, '0' | 'O' | '1' | 'I' | 'l'),
                "ambiguous char in {id}"
            );
        }
    }
}
