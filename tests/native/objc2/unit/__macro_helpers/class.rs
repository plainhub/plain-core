
    use super::*;
    use crate::extern_class;
    use crate::runtime::NSObject;

    extern_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = AnyThread]
        #[name = "NSObject"]
        struct SetAnyThread;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = AnyThread]
        #[name = "NSObject"]
        struct SendSync;
    );

    unsafe impl Send for SendSync {}
    unsafe impl Sync for SendSync {}

    extern_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "NSObject"]
        struct OnlyMain;
    );

    extern_class!(
        #[unsafe(super(OnlyMain))]
        #[name = "NSObject"]
        struct OnlyMainSubDefault;
    );

    extern_class!(
        #[unsafe(super(OnlyMain))]
        #[thread_kind = MainThreadOnly]
        #[name = "NSObject"]
        struct OnlyMainSubExplicit;
    );

