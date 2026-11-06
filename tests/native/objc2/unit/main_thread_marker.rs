
    use super::*;
    use std::panic::{RefUnwindSafe, UnwindSafe};

    static_assertions::assert_impl_all!(MainThreadMarker: Unpin, UnwindSafe, RefUnwindSafe, Sized);
    static_assertions::assert_not_impl_any!(MainThreadMarker: Send, Sync);

    #[test]
    fn debug() {
        // SAFETY: We don't use the marker for anything other than its Debug
        // impl, so this test doesn't actually need to run on the main thread!
        let marker = unsafe { MainThreadMarker::new_unchecked() };
        assert_eq!(std::format!("{marker:?}"), "MainThreadMarker");
    }

    #[test]
    fn test_not_main_thread() {
        let res = std::thread::spawn(|| MainThreadMarker::new().is_none())
            .join()
            .unwrap();
        assert!(res);
    }

