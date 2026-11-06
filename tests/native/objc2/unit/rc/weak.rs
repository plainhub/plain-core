
    use core::mem;

    use super::*;
    use crate::rc::{RcTestObject, ThreadTestData};
    use crate::runtime::NSObject;

    #[test]
    fn test_weak() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let weak = Weak::from(&obj);
        expected.assert_current();

        let strong = weak.load().unwrap();
        expected.try_retain += 1;
        expected.assert_current();
        assert!(ptr::eq(&*strong, &*obj));

        drop(obj);
        drop(strong);
        expected.release += 2;
        expected.drop += 1;
        expected.assert_current();

        if cfg!(not(any())) {
            // This loads the object on GNUStep for some reason??
            assert!(weak.load().is_none());
            expected.assert_current();
        }

        drop(weak);
        expected.assert_current();
    }

    #[test]
    fn test_weak_clone() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let weak = Weak::from(&obj);
        expected.assert_current();

        let weak2 = weak.clone();
        if cfg!(target_vendor = "apple") {
            expected.try_retain += 1;
            expected.release += 1;
        }
        expected.assert_current();

        let strong = weak.load().unwrap();
        expected.try_retain += 1;
        expected.assert_current();
        assert!(ptr::eq(&*strong, &*obj));

        let strong2 = weak2.load().unwrap();
        expected.try_retain += 1;
        expected.assert_current();
        assert!(ptr::eq(&*strong, &*strong2));

        drop(weak);
        drop(weak2);
        expected.assert_current();
    }

    #[test]
    fn test_weak_default() {
        let weak: Weak<RcTestObject> = Weak::default();
        assert!(weak.load().is_none());
        drop(weak);
    }

    #[repr(C)]
    struct MyObject<'a> {
        inner: NSObject,
        p: PhantomData<&'a str>,
    }

    /// Test that `Weak<T>` is covariant over `T`.
    #[allow(unused)]
    fn assert_variance<'a, 'b>(obj: &'a Weak<MyObject<'static>>) -> &'a Weak<MyObject<'b>> {
        obj
    }

    #[test]
    fn test_size_of() {
        assert_eq!(
            mem::size_of::<Option<Weak<NSObject>>>(),
            mem::size_of::<*const ()>()
        );
    }

