
    use crate::runtime::NSObject;
    use crate::{extern_class, extern_protocol};

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "NSObject"]
        struct OldSyntax;
    );

    extern_protocol!(
        #[name = "NSObjectProtocol"]
        #[allow(clippy::missing_safety_doc)]
        unsafe trait Protocol {}
    );

    // Old syntax, must continue to work until next breaking version.
    unsafe impl Protocol for OldSyntax {}

