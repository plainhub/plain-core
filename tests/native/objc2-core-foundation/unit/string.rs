
    use alloc::string::ToString;
    use core::ffi::CStr;

    use super::*;

    #[test]
    fn basic_conversion() {
        let s = CFString::from_str("abc");
        assert_eq!(s.to_string(), "abc");
        let s = CFString::from_str("a♥😀");
        assert_eq!(s.to_string(), "a♥😀");
    }

    #[test]
    fn cstr_conversion() {
        let table = [
            (
                b"abc\xf8xyz\0" as &[u8],
                CFStringBuiltInEncodings::EncodingISOLatin1,
                "abcøxyz",
            ),
            (
                b"\x26\x65\0",
                CFStringBuiltInEncodings::EncodingUTF16BE,
                "♥",
            ),
            (
                b"\x65\x26\0",
                CFStringBuiltInEncodings::EncodingUTF16LE,
                "♥",
            ),
        ];
        for (cstr, encoding, expected) in table {
            let cstr = CStr::from_bytes_with_nul(cstr).unwrap();
            let s = unsafe { CFString::with_c_string(None, cstr.as_ptr(), encoding.0) }.unwrap();
            assert_eq!(s.to_string(), expected);
        }
    }

    #[test]
    fn from_incomplete() {
        let s = unsafe {
            CFString::with_bytes(
                None,
                b"\xd8\x3d\xde".as_ptr(),
                3,
                CFStringBuiltInEncodings::EncodingUTF16BE.0,
                false,
            )
            .unwrap()
        };
        assert_eq!(s.to_string(), "�"); // Replacement character
        assert_eq!(s.length(), 1);
    }

    #[test]
    fn internal_nul_byte() {
        let s = CFString::from_str("a\0b\0c\0d");
        // Works with `CFStringGetBytes`.
        assert_eq!(s.to_string(), "a\0b\0c\0d");
        // `CFStringGetCStringPtr` does not seem to work here on very short
        // strings (probably those that are stored inline?).
        if cfg!(target_pointer_width = "64") {
            assert_eq!(unsafe { s.as_str_unchecked() }, None);
        } else {
            assert_eq!(unsafe { s.as_str_unchecked() }, Some("a\0b\0c\0d"));
        }

        // Test `CFStringGetCString`.
        let mut buf = [0u8; 10];
        assert!(unsafe {
            s.c_string(
                buf.as_mut_ptr().cast(),
                buf.len() as _,
                CFStringBuiltInEncodings::EncodingUTF8.0,
            )
        });
        // All the data is copied to the buffer.
        assert_eq!(&buf[0..10], b"a\0b\0c\0d\0\0\0");

        // But subsequent usage of that as a CStr fails, since it contains
        // interior NUL bytes.
        let cstr = CStr::from_bytes_until_nul(&buf).unwrap();
        assert_eq!(cstr.to_bytes(), b"a");

        // Test with a bit longer string, to ensure the same holds for heap-
        // allocated CFStrings
        let s = CFString::from_str("a\0aaaaaaaaaaaaaaa");
        // Works with `CFStringGetBytes`.
        assert_eq!(s.to_string(), "a\0aaaaaaaaaaaaaaa");
        // `CFStringGetCStringPtr` also allows these without truncation.
        assert_eq!(unsafe { s.as_str_unchecked() }, Some("a\0aaaaaaaaaaaaaaa"));
    }

    #[test]
    fn as_str_correct_on_unicode() {
        let s = CFString::from_static_str("😀");
        assert_eq!(unsafe { s.as_str_unchecked() }, None);
        let s = CFString::from_static_str("♥");
        assert_eq!(unsafe { s.as_str_unchecked() }, None);
    }

    #[test]
    fn utf8_on_boundary() {
        // Make the emoji lie across the 32 byte buffer size in Display::fmt.
        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaa😀"; // 29 'a's
        assert_eq!(CFString::from_str(s).to_string(), s);
        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa😀"; // 30 'a's
        assert_eq!(CFString::from_str(s).to_string(), s);
        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa😀"; // 31 'a's
        assert_eq!(CFString::from_str(s).to_string(), s);
    }

    #[test]
    fn create_with_cstring_broken_on_non_8_bit() {
        // A CFString that is supposed to contain a "♥" (the UTF-8 encoding of
        // that is the vastly different b"\xE2\x99\xA5").
        //
        // This line is wrong, because `CFStringCreateWithCString` expects an
        // 8-bit encoding.
        //
        // See also:
        // https://github.com/swiftlang/swift-corelibs-foundation/issues/5164
        let s = unsafe {
            CFString::with_c_string(
                None,
                b"\x65\x26\0".as_ptr().cast(),
                CFStringBuiltInEncodings::EncodingUnicode.0,
            )
        }
        .unwrap();

        // `CFStringGetBytes` used in `fmt::Display` converts to UTF-8.
        assert_eq!(s.to_string(), "♥");

        // So does `CFStringGetCString`.
        let mut buf = [0u8; 20];
        assert!(unsafe {
            s.c_string(
                buf.as_mut_ptr().cast(),
                buf.len() as _,
                CFStringBuiltInEncodings::EncodingUTF8.0,
            )
        });
        let cstr = CStr::from_bytes_until_nul(&buf).unwrap();
        assert_eq!(cstr.to_bytes(), "♥".as_bytes());

        // `CFStringGetCStringPtr` completely ignores the requested UTF-8 conversion.
        assert_eq!(unsafe { s.as_str_unchecked() }, Some("e"));
        assert_eq!(
            unsafe { CStr::from_ptr(s.c_string_ptr(CFStringBuiltInEncodings::EncodingUTF8.0,)) },
            CStr::from_bytes_with_nul(b"e&\0").unwrap()
        );
    }

    #[test]
    fn test_static() {
        let cf = CFString::from_static_str("xyz");
        assert_eq!(cf.to_string(), "xyz");
    }

    #[test]
    fn eq() {
        assert_eq!(CFString::from_str("abc"), CFString::from_str("abc"));
        assert_ne!(CFString::from_str("abc"), CFString::from_str("xyz"));
        // Cross-type comparison
        assert_ne!(
            **CFString::from_str("abc"),
            **unsafe { kCFAllocatorNull }.unwrap()
        );
    }

    // TODO: Test mutation while formatting.

