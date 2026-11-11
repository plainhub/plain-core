use super::*;
thread_local! { static PAIRED_ITERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub(super) fn record_paired_iteration() {
    PAIRED_ITERATIONS.set(PAIRED_ITERATIONS.get() + 1);
}
#[cfg(target_arch = "aarch64")]
type NativeVector = core::arch::aarch64::uint8x16_t;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
type NativeVector = core::arch::x86_64::__m128i;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
type NativeVector = core::arch::wasm32::v128;
#[cfg(any(
    target_arch = "aarch64",
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128")
))]
#[test]
fn paired_vector_chunks_preserve_first_match_and_partial_tail_without_overread() {
    let needle = b"abaz";
    let finder = unsafe { Finder::<NativeVector>::new(needle, Pair::new(needle).unwrap()) };
    for alignment in 0..32 {
        for length in [64, 65, 79, 80, 81, 127, 128, 129, 255, 256, 257] {
            let mut storage = vec![b'q'; alignment + length + 32];
            storage[alignment + length..alignment + length + needle.len()].copy_from_slice(needle);
            if alignment >= needle.len() {
                storage[alignment - needle.len()..alignment].copy_from_slice(needle);
            }
            let haystack = &mut storage[alignment..alignment + length];
            PAIRED_ITERATIONS.set(0);
            assert_eq!(unsafe { finder.find(haystack, needle) }, None);
            assert!(PAIRED_ITERATIONS.get() > 0);
            PAIRED_ITERATIONS.set(0);
            assert_eq!(unsafe { finder.find_prefilter(haystack) }, None);
            assert!(PAIRED_ITERATIONS.get() > 0);
            for position in [0, 15, 16, 17, 31, 32, length - needle.len()] {
                haystack.fill(b'q');
                haystack[position..position + needle.len()].copy_from_slice(needle);
                assert_eq!(unsafe { finder.find(haystack, needle) }, Some(position));
            }
            haystack.fill(b'q');
            haystack[16..20].copy_from_slice(needle);
            haystack[32..36].copy_from_slice(needle);
            assert_eq!(unsafe { finder.find(haystack, needle) }, Some(16));
        }
    }
}
