use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Element {
    clones: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

impl Clone for Element {
    fn clone(&self) -> Self {
        if self.clones.fetch_add(1, Ordering::SeqCst) == 2 {
            panic!("fixture clone failure");
        }
        Self {
            clones: Arc::clone(&self.clones),
            drops: Arc::clone(&self.drops),
        }
    }
}

impl Drop for Element {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn a_panicking_clone_drops_every_initialized_element() {
    let clones = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let element = Element {
        clones: Arc::clone(&clones),
        drops: Arc::clone(&drops),
    };
    let result = std::panic::catch_unwind(|| fallible_vec_from_element(element, 8));
    assert!(result.is_err());
    assert_eq!(clones.load(Ordering::SeqCst), 3);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}

#[test]
fn allocation_limits_still_reject_oversized_items() {
    let original = crate::audio_tags::config::global_options();
    crate::audio_tags::config::apply_global_options(
        crate::audio_tags::config::GlobalOptions::new().allocation_limit(2),
    );
    assert!(fallible_vec_from_element(0_u8, 3).is_err());
    assert_eq!(fallible_vec_from_element(7_u8, 2).unwrap(), [7, 7]);
    crate::audio_tags::config::apply_global_options(original);
}
