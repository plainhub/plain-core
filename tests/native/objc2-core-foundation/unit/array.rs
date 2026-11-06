
    use super::*;
    #[cfg(feature = "CFString")]
    use crate::CFString;
    use core::ptr::null;

    #[test]
    fn array_with_invalid_pointers() {
        // without_provenance
        let ptr = [0 as _, 1 as _, 2 as _, 3 as _, usize::MAX as _].as_mut_ptr();
        let array = unsafe { CFArray::new(None, ptr, 1, null()) }.unwrap();
        let value = unsafe { array.value_at_index(0) };
        assert!(value.is_null());
    }

    #[test]
    #[should_panic]
    #[ignore = "aborts (as expected)"]
    fn object_array_cannot_contain_null() {
        let ptr = [null()].as_mut_ptr();
        let _array = unsafe { CFArray::new(None, ptr, 1, &kCFTypeArrayCallBacks) };
    }

    #[test]
    #[cfg(feature = "CFString")]
    fn correct_retain_count() {
        let objects = [
            CFString::from_str("some long string that doesn't get small-string optimized"),
            CFString::from_str("another long string that doesn't get small-string optimized"),
        ];
        let array = CFArray::from_retained_objects(&objects);

        // Creating array retains elements.
        assert_eq!(array.retain_count(), 1);
        assert_eq!(unsafe { array.get_unchecked(0) }.retain_count(), 2);
        assert_eq!(unsafe { array.get_unchecked(1) }.retain_count(), 2);

        drop(objects);
        assert_eq!(unsafe { array.get_unchecked(0) }.retain_count(), 1);
        assert_eq!(unsafe { array.get_unchecked(1) }.retain_count(), 1);

        // Retaining array doesn't affect retain count of elements.
        let _array2 = array.retain();
        assert_eq!(unsafe { array.get_unchecked(0) }.retain_count(), 1);
        assert_eq!(unsafe { array.get_unchecked(1) }.retain_count(), 1);

        // Using retaining API changes retain count.
        assert_eq!(array.get(0).unwrap().retain_count(), 2);
    }

    #[test]
    #[cfg(feature = "CFString")]
    fn iter() {
        use alloc::vec::Vec;

        let s1 = CFString::from_str("a");
        let s2 = CFString::from_str("b");
        let array = CFArray::from_objects(&[&*s1, &*s2, &*s1]);

        assert_eq!(
            array.iter().collect::<Vec<_>>(),
            [s1.clone(), s2.clone(), s1.clone()]
        );

        assert_eq!(
            unsafe { array.iter_unchecked() }.collect::<Vec<_>>(),
            [&*s1, &*s2, &*s1]
        );
    }

    #[test]
    #[cfg(feature = "CFString")]
    fn iter_fused() {
        let s1 = CFString::from_str("a");
        let s2 = CFString::from_str("b");
        let array = CFArray::from_objects(&[&*s1, &*s2]);

        let mut iter = array.iter();
        assert_eq!(iter.next(), Some(s1.clone()));
        assert_eq!(iter.next(), Some(s2.clone()));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);

        let mut iter = unsafe { array.iter_unchecked() };
        assert_eq!(iter.next(), Some(&*s1));
        assert_eq!(iter.next(), Some(&*s2));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    #[cfg(feature = "CFString")]
    fn mutate() {
        let array = CFMutableArray::<CFString>::with_capacity(10);
        array.insert(0, &CFString::from_str("a"));
        array.append(&CFString::from_str("c"));
        array.insert(1, &CFString::from_str("b"));
        assert_eq!(
            array.to_vec(),
            [
                CFString::from_str("a"),
                CFString::from_str("b"),
                CFString::from_str("c"),
            ]
        );
    }

    #[test]
    #[cfg(feature = "CFString")]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "not detected when debug assertions are off"
    )]
    #[should_panic = "array was mutated while iterating"]
    fn mutate_while_iter_unchecked() {
        let array = CFMutableArray::<CFString>::with_capacity(10);
        assert_eq!(array.len(), 0);

        let mut iter = unsafe { array.iter_unchecked() };
        array.append(&CFString::from_str("a"));
        // Should panic
        let _ = iter.next();
    }

