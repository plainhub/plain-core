
    use alloc::string::ToString;
    use core::ptr;

    use super::*;
    use crate::msg_send;
    use crate::rc::Allocated;
    use crate::runtime::NSObject;
    use crate::ClassType;

    #[test]
    fn alloc_with_zone() {
        let zone: *const NSZone = ptr::null();
        let _obj: Allocated<NSObject> =
            unsafe { msg_send![NSObject::class(), allocWithZone: zone] };
    }

    #[test]
    fn verify_encoding() {
        let expected = if cfg!(all(any(), target_pointer_width = "64")) {
            "^{_NSZone=^?^?^?^?^?^?^?Q@^{_NSZone}}"
        } else if cfg!(all(any(), not(target_pointer_width = "64"))) {
            "^{_NSZone=^?^?^?^?^?^?^?I@^{_NSZone}}"
        } else {
            "^{_NSZone=}"
        };
        assert_eq!(NSZone::ENCODING_REF.to_string(), expected);
    }

