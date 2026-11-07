use super::*;

#[test]
fn test_nfkc() {
    let original = "\u{fb01}".to_string();
    let normalized = "fi".to_string();
    let mut n = NormalizedString::from(original.clone());
    NFKC.normalize(&mut n).unwrap();

    assert_eq!(
        n,
        NormalizedString::new(original, normalized, vec![(0, 3), (0, 3)], 0)
    );

    assert_eq!(n.alignments_original(), vec![(0, 2), (0, 2), (0, 2)]);
}
