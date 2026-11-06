
    use super::*;
    use crate::runtime::NSObject;
    use crate::{define_class, msg_send, ClassType};

    define_class!(
        #[unsafe(super(NSObject))]
        #[derive(PartialEq, Eq, Hash, Debug)]
        struct Collection;
    );

    impl DefaultRetained for Collection {
        fn default_retained() -> Retained<Self> {
            unsafe { msg_send![Collection::class(), new] }
        }
    }

    struct Iter<'a> {
        _inner: &'a Collection,
    }

    impl<'a> Iterator for Iter<'a> {
        type Item = &'a NSObject;
        fn next(&mut self) -> Option<Self::Item> {
            None
        }
    }

    impl<'a> IntoIterator for &'a Collection {
        type Item = &'a NSObject;
        type IntoIter = Iter<'a>;

        fn into_iter(self) -> Self::IntoIter {
            Iter { _inner: self }
        }
    }

    struct IntoIter {
        _inner: Retained<Collection>,
    }

    impl Iterator for IntoIter {
        type Item = Retained<NSObject>;
        fn next(&mut self) -> Option<Self::Item> {
            None
        }
    }

    impl RetainedIntoIterator for Collection {
        type Item = Retained<NSObject>;
        type IntoIter = IntoIter;

        fn retained_into_iter(this: Retained<Self>) -> Self::IntoIter {
            IntoIter { _inner: this }
        }
    }

    impl RetainedFromIterator<Retained<NSObject>> for Collection {
        fn retained_from_iter<I: IntoIterator<Item = Retained<NSObject>>>(
            _iter: I,
        ) -> Retained<Self> {
            Collection::default_retained()
        }
    }

    #[test]
    fn test_default() {
        let obj1: Retained<Collection> = Default::default();
        let obj2 = Collection::default_retained();
        assert_ne!(obj1, obj2);
    }

    #[test]
    fn test_into_iter() {
        let obj: Retained<Collection> = Default::default();

        for _ in &*obj {}
        for _ in &obj {}

        for _ in obj {}
    }

    #[test]
    fn test_from_iter() {
        let _: Retained<Collection> = [NSObject::new()].into_iter().collect();
    }

