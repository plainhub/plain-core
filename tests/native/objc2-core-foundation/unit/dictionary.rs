
    use super::*;
    use crate::CFType;

    #[test]
    fn empty() {
        let dict1 = CFDictionary::<CFType, CFType>::empty();
        let dict2 = CFDictionary::<CFType, CFType>::empty();
        assert_eq!(dict1, dict2);
        assert_eq!(dict1.len(), 0);
        assert_eq!(dict1.get(dict1.as_ref()), None);
        assert!(!dict1.contains_key(dict1.as_ref()));
        assert!(!dict1.contains_value(dict1.as_ref()));
        #[cfg(feature = "alloc")]
        assert_eq!(dict1.to_vecs(), (alloc::vec![], alloc::vec![]));
    }

    #[test]
    #[cfg(feature = "CFString")]
    fn mutable_dictionary() {
        use crate::CFString;

        let dict = CFMutableDictionary::<CFString, CFString>::empty();
        dict.add(&CFString::from_str("a"), &CFString::from_str("b"));
        dict.add(&CFString::from_str("c"), &CFString::from_str("d"));
        assert_eq!(dict.len(), 2);

        dict.add(&CFString::from_str("c"), &CFString::from_str("e"));
        assert_eq!(dict.len(), 2);
        assert_eq!(
            dict.get(&CFString::from_str("c")),
            Some(CFString::from_str("d")),
        );

        dict.replace(&CFString::from_str("c"), &CFString::from_str("f"));
        assert_eq!(dict.len(), 2);
        assert_eq!(
            dict.get(&CFString::from_str("c")),
            Some(CFString::from_str("f"))
        );

        dict.remove(&CFString::from_str("a"));
        assert_eq!(dict.len(), 1);

        dict.clear();
        assert_eq!(dict.len(), 0);
    }

    #[test]
    #[cfg(feature = "CFString")]
    fn contains() {
        use crate::CFString;

        let dict = CFDictionary::from_slices(
            &[&*CFString::from_str("key")],
            &[&*CFString::from_str("value")],
        );

        assert!(dict.contains_key(&CFString::from_str("key")));
        assert!(dict.get(&CFString::from_str("key")).is_some());

        assert!(!dict.contains_key(&CFString::from_str("invalid key")));
        assert!(dict.get(&CFString::from_str("invalid key")).is_none());

        assert!(dict.contains_value(&CFString::from_str("value")));
        assert!(!dict.contains_value(&CFString::from_str("invalid value")));
    }

    #[test]
    #[cfg(all(feature = "CFString", any()))]
    fn heterogenous() {
        use crate::{CFBoolean, CFNumber, CFString, CFType};

        let dict = CFDictionary::<CFType, CFType>::from_slices(
            &[
                CFString::from_str("string key").as_ref(),
                CFNumber::new_isize(4).as_ref(),
                CFBoolean::new(true).as_ref(),
            ],
            &[
                CFString::from_str("a string value").as_ref(),
                CFNumber::new_isize(2).as_ref(),
                CFBoolean::new(false).as_ref(),
            ],
        );

        assert_eq!(
            dict.get(&CFString::from_str("string key")),
            Some(CFString::from_str("a string value").into())
        );
        assert_eq!(
            dict.get(&CFNumber::new_isize(4)),
            Some(CFNumber::new_isize(2).into())
        );
        assert_eq!(
            dict.get(CFBoolean::new(true)),
            Some(CFBoolean::new(false).into())
        );
        assert_eq!(dict.get(CFBoolean::new(false)), None);
    }

    #[test]
    #[cfg(feature = "CFString")]
    #[should_panic = "key and object slices must have the same length"]
    fn from_slices_not_same_length() {
        use crate::CFString;
        let _dict =
            CFDictionary::<CFString, CFString>::from_slices(&[&*CFString::from_str("key")], &[]);
    }

