use super::*;
use std::hash::{BuildHasher, Hasher};
#[test]
fn one_state_preserves_its_seed_and_distinguishes_inputs() {
    let state = fast::RandomState::default();
    let mut values = std::collections::HashSet::new();
    for n in 0..1024_u64 {
        let mut a = state.build_hasher();
        a.write_u64(n);
        let mut b = state.build_hasher();
        b.write_u64(n);
        assert_eq!(a.finish(), b.finish());
        assert!(values.insert(a.finish()));
    }
}
