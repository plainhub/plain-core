use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn incomplete_reference_chunks_preserve_the_tail() {
    for length in 0..12 {
        let values: Vec<_> = (0..length).collect();
        let mut chunks = ArrayChunks::<_, 4>::new(values.iter());
        assert_eq!(chunks.len(), length / 4);
        assert_eq!(chunks.size_hint(), (length / 4, Some(length / 4)));
        let complete: Vec<_> = chunks.by_ref().collect();
        assert_eq!(complete.len(), length / 4);
        assert_eq!(chunks.next(), None);
        let remainder: Vec<_> = chunks.into_remainder().unwrap().copied().collect();
        assert_eq!(remainder, values[(length / 4) * 4..]);
    }
}

#[test]
fn mutable_row_references_remain_disjoint() {
    let mut pixels = [0, 1, 2, 3, 4];
    let mut chunks = ArrayChunks::<_, 2>::new(pixels.iter_mut());
    for [left, right] in chunks.by_ref() {
        *left += 10;
        *right += 20;
    }
    *chunks.into_remainder().unwrap().next().unwrap() += 30;
    assert_eq!(pixels, [10, 21, 12, 23, 34]);
}

struct Item(Arc<AtomicUsize>);

impl Drop for Item {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn a_panicking_input_drops_the_initialized_prefix() {
    let drops = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&drops);
    let result = std::panic::catch_unwind(move || {
        let input = (0..4).map(|index| {
            assert_ne!(index, 2, "fixture iterator failure");
            Item(Arc::clone(&observed))
        });
        ArrayChunks::<_, 4>::new(input).next();
    });
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
#[should_panic(expected = "chunk size must be non-zero")]
fn zero_sized_chunks_are_rejected() {
    let _ = ArrayChunks::<_, 0>::new(0..4);
}
