
    use super::*;
    use core::ffi::CStr;

    #[test]
    fn smoke() {
        // Verify that this library links and works fine by itself
        let name = CStr::from_bytes_with_nul(b"abc:def:\0").unwrap();
        let sel = unsafe { sel_registerName(name.as_ptr()).unwrap() };
        let rtn = unsafe { CStr::from_ptr(sel_getName(sel)) };
        assert_eq!(name, rtn);
    }

