
    use super::*;

    #[test]
    #[allow(unused_unsafe)]
    fn test_isa() {
        let isa = Isa(unsafe { ptr::addr_of!(ffi::_NSConcreteGlobalBlock) });
        assert!(isa.is_global());
        assert!(!isa.is_stack());
        let isa = Isa(unsafe { ptr::addr_of!(ffi::_NSConcreteStackBlock) });
        assert!(!isa.is_global());
        assert!(isa.is_stack());
        let isa = Isa(unsafe { ptr::addr_of!(ffi::private::_NSConcreteMallocBlock) });
        assert!(!isa.is_global());
        assert!(!isa.is_stack());
        let isa = Isa(ptr::null());
        assert!(!isa.is_global());
        assert!(!isa.is_stack());
    }

