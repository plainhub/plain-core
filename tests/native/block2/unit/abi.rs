
    use super::*;

    fn assert_no_trailing_padding<T>() {
        struct AddU8<T> {
            t: T,
            extra: u8,
        }
        assert_ne!(core::mem::size_of::<T>(), core::mem::size_of::<AddU8<T>>());
    }

    #[test]
    fn no_types_have_padding() {
        assert_no_trailing_padding::<BlockHeader>();
        assert_no_trailing_padding::<BlockDescriptorPtr>();
        assert_no_trailing_padding::<BlockDescriptor>();
        assert_no_trailing_padding::<BlockDescriptorCopyDispose>();
        assert_no_trailing_padding::<BlockDescriptorSignature>();
        assert_no_trailing_padding::<BlockDescriptorCopyDisposeSignature>();
    }

