
    use core::cell::Cell;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Test that the way you specify lifetimes are as documented in the
    /// reference.
    /// <https://doc.rust-lang.org/nightly/reference/lifetime-elision.html#default-trait-object-lifetimes>
    #[test]
    fn test_rust_dyn_lifetime_semantics() {
        fn takes_static(block: &Block<dyn Fn() + 'static>) {
            block.call(());
        }

        fn takes_elided(block: &Block<dyn Fn() + '_>) {
            block.call(());
        }

        fn takes_unspecified(block: &Block<dyn Fn()>) {
            block.call(());
        }

        // Static lifetime
        static MY_STATIC: AtomicUsize = AtomicUsize::new(0);
        MY_STATIC.store(0, Ordering::Relaxed);
        let static_lifetime: RcBlock<dyn Fn() + 'static> = RcBlock::new(|| {
            MY_STATIC.fetch_add(1, Ordering::Relaxed);
        });
        takes_static(&static_lifetime);
        takes_elided(&static_lifetime);
        takes_unspecified(&static_lifetime);
        assert_eq!(MY_STATIC.load(Ordering::Relaxed), 3);

        // Lifetime declared with `'_`
        let captured = Cell::new(0);
        let elided_lifetime: RcBlock<dyn Fn() + '_> = RcBlock::new(|| {
            captured.set(captured.get() + 1);
        });
        // takes_static(&elided_lifetime); // Compile error
        takes_elided(&elided_lifetime);
        // takes_unspecified(&elided_lifetime); // Compile error
        assert_eq!(captured.get(), 1);

        // Lifetime kept unspecified
        let captured = Cell::new(0);
        let unspecified_lifetime: RcBlock<dyn Fn()> = RcBlock::new(|| {
            captured.set(captured.get() + 1);
        });
        // takes_static(&unspecified_lifetime); // Compile error
        takes_elided(&unspecified_lifetime);
        // takes_unspecified(&unspecified_lifetime); // Compile error
        assert_eq!(captured.get(), 1);
    }

    #[allow(dead_code)]
    fn unspecified_in_fn_is_static(block: &Block<dyn Fn()>) -> &Block<dyn Fn() + 'static> {
        block
    }

    #[allow(dead_code)]
    fn lending_block<'b>(block: &Block<dyn Fn() -> &'b i32 + 'b>) {
        let _ = *block.call(());
    }

    #[allow(dead_code)]
    fn takes_lifetime(_: &Block<dyn Fn(&i32) -> &i32>) {
        // Not actually callable yet
    }

    #[allow(dead_code)]
    fn covariant<'b, 'f>(b: &'b Block<dyn Fn() + 'static>) -> &'b Block<dyn Fn() + 'f> {
        b
    }

