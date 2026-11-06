
    use super::*;
    use core::ptr;

    #[test]
    fn smoke() {
        assert_eq!(unsafe { _Block_copy(ptr::null_mut()) }, ptr::null_mut());
        unsafe { _Block_release(ptr::null_mut()) };
    }

    #[test]
    #[allow(unused_unsafe)]
    #[cfg(feature = "std")]
    fn test_linkable() {
        use std::println;
        println!("{:?}", unsafe { ptr::addr_of!(_NSConcreteGlobalBlock) });
        println!("{:?}", unsafe { ptr::addr_of!(_NSConcreteStackBlock) });
        println!("{:?}", unsafe {
            ptr::addr_of!(private::_NSConcreteMallocBlock)
        });
        println!("{:p}", _Block_copy as unsafe extern "C-unwind" fn(_) -> _);
        println!(
            "{:p}",
            _Block_object_assign as unsafe extern "C-unwind" fn(_, _, _)
        );
        println!(
            "{:p}",
            _Block_object_dispose as unsafe extern "C-unwind" fn(_, _)
        );
        println!("{:p}", _Block_release as unsafe extern "C-unwind" fn(_));
    }

