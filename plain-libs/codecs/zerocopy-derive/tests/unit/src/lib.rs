use super::*;

#[test]
fn test_config_repr_orderings() {
    // Validate that the repr lists in the various configs are in the
    // canonical order. If they aren't, then our algorithm to look up in
    // those lists won't work.

    // TODO(https://github.com/rust-lang/rust/issues/53485): Remove once
    // `Vec::is_sorted` is stabilized.
    fn is_sorted_and_deduped<T: Clone + Ord>(ts: &[T]) -> bool {
        let mut sorted = ts.to_vec();
        sorted.sort();
        sorted.dedup();
        ts == sorted.as_slice()
    }

    fn elements_are_sorted_and_deduped<T: Clone + Ord>(lists: &[&[T]]) -> bool {
        lists.iter().all(|list| is_sorted_and_deduped(list))
    }

    fn config_is_sorted<T: KindRepr + Clone>(config: &Config<T>) -> bool {
        elements_are_sorted_and_deduped(config.allowed_combinations)
            && elements_are_sorted_and_deduped(config.disallowed_but_legal_combinations)
    }

    assert!(config_is_sorted(&STRUCT_UNION_UNALIGNED_CFG));
    assert!(config_is_sorted(&ENUM_FROM_BYTES_CFG));
    assert!(config_is_sorted(&ENUM_UNALIGNED_CFG));
}

#[test]
fn test_config_repr_no_overlap() {
    // Validate that no set of reprs appears in both the
    // `allowed_combinations` and `disallowed_but_legal_combinations` lists.

    fn overlap<T: Eq>(a: &[T], b: &[T]) -> bool {
        a.iter().any(|elem| b.contains(elem))
    }

    fn config_overlaps<T: KindRepr + Eq>(config: &Config<T>) -> bool {
        overlap(
            config.allowed_combinations,
            config.disallowed_but_legal_combinations,
        )
    }

    assert!(!config_overlaps(&STRUCT_UNION_UNALIGNED_CFG));
    assert!(!config_overlaps(&ENUM_FROM_BYTES_CFG));
    assert!(!config_overlaps(&ENUM_UNALIGNED_CFG));
}
