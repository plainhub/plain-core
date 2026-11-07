use super::*;

#[test]
fn test_prepend() {
    let original = "Hello";
    let normalized = "▁Hello";
    assert_ne!(original, normalized);
    let mut n = NormalizedString::from(original);
    let prepend = Prepend::new("▁".to_string());
    prepend.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);
    assert_eq!(
        n,
        NormalizedString::new(
            original.to_string(),
            normalized.to_string(),
            vec![
                (0, 1),
                (0, 1),
                (0, 1),
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5)
            ],
            0
        )
    );
    assert_eq!(
        n.alignments_original(),
        vec![(0, 4), (4, 5), (5, 6), (6, 7), (7, 8)]
    );
}
