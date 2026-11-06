
    use core::ffi::c_char;

    use super::*;

    #[test]
    fn test_manual_block_encoding_is_none() {
        // Normal case.
        struct Enc1;
        unsafe impl ManualBlockEncoding for Enc1 {
            type Arguments = (i32, f32);
            type Return = u8;
            #[cfg(target_pointer_width = "64")]
            const ENCODING_CSTR: &'static CStr =
                // Somehow, using a C string literal seems to fail the MSRV
                // check here, so use the old way instead here.
                unsafe { CStr::from_bytes_with_nul_unchecked(b"C16@?0i8f12\0") };
            #[cfg(not(target_pointer_width = "64"))]
            const ENCODING_CSTR: &'static CStr =
                unsafe { CStr::from_bytes_with_nul_unchecked(b"C12@?0i4f8\0") };
        }
        // HACK: use `identity` in order to circumvent a Clippy warning.
        assert!(!core::convert::identity(UserSpecified::<Enc1>::IS_NONE));

        // No input + no output case.
        struct Enc2;
        unsafe impl ManualBlockEncoding for Enc2 {
            type Arguments = ();
            type Return = ();
            #[cfg(target_pointer_width = "64")]
            const ENCODING_CSTR: &'static CStr =
                unsafe { CStr::from_bytes_with_nul_unchecked(b"v8@?0\0") };
            #[cfg(not(target_pointer_width = "64"))]
            const ENCODING_CSTR: &'static CStr =
                unsafe { CStr::from_bytes_with_nul_unchecked(b"v4@?0\0") };
        }
        assert!(!core::convert::identity(UserSpecified::<Enc2>::IS_NONE));

        // Ensure we don't rely on the encoding string's emptiness.
        struct Enc3;
        unsafe impl ManualBlockEncoding for Enc3 {
            type Arguments = ();
            type Return = ();
            const ENCODING_CSTR: &'static CStr =
                unsafe { CStr::from_bytes_with_nul_unchecked(b"\0") };
        }
        assert!(!core::convert::identity(UserSpecified::<Enc3>::IS_NONE));

        // Only `NoBlockEncoding` should be `IS_NONE`.
        assert!(core::convert::identity(NoBlockEncoding::<(), ()>::IS_NONE));
        assert!(core::convert::identity(
            NoBlockEncoding::<(i32, f32), u8>::IS_NONE
        ));
        assert!(core::convert::identity(
            NoBlockEncoding::<(*const u8,), *const c_char>::IS_NONE
        ));
    }

