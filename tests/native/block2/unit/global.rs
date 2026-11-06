
    use super::*;
    use alloc::format;

    global_block! {
        /// Test comments and visibility
        pub(super) static NOOP_BLOCK = || {};
    }

    global_block! {
        /// Multiple parameters + trailing comma
        #[allow(unused)]
        static BLOCK = |x: i32, y: i32, z: i32, w: i32,| -> i32 {
            x + y + z + w
        };
    }

    #[test]
    fn test_noop_block() {
        NOOP_BLOCK.call(());
    }

    #[test]
    fn test_defined_in_function() {
        global_block!(static MY_BLOCK = || -> i32 {
            42
        });
        assert_eq!(MY_BLOCK.call(()), 42);
    }

    #[cfg(target_vendor = "apple")]
    const DEBUG_BLOCKFLAGS: &str = r#"BlockFlags {
        value: "00110000000000000000000000000000",
        deallocating: false,
        inline_layout_string: false,
        small_descriptor: false,
        is_noescape: false,
        needs_free: false,
        has_copy_dispose: false,
        has_ctor: false,
        is_gc: false,
        is_global: true,
        use_stret: true,
        has_signature: false,
        has_extended_layout: false,
        over_referenced: false,
        reference_count: 0,
        ..
    }"#;

    #[cfg(not(target_vendor = "apple"))]
    const DEBUG_BLOCKFLAGS: &str = r#"BlockFlags {
        value: "00110000000000000000000000000000",
        has_copy_dispose: false,
        has_ctor: false,
        is_global: true,
        use_stret: true,
        has_signature: false,
        over_referenced: false,
        reference_count: 0,
        ..
    }"#;

    #[test]
    fn test_debug() {
        let invoke = NOOP_BLOCK.header.invoke.unwrap();
        let size = mem::size_of::<BlockHeader>();
        let maybeuninit = <MaybeUninit<i32>>::uninit();
        let expected = format!(
            "GlobalBlock {{
    isa: _NSConcreteGlobalBlock,
    flags: {DEBUG_BLOCKFLAGS},
    reserved: {maybeuninit:?},
    invoke: Some(
        {invoke:#?},
    ),
    descriptor: BlockDescriptor {{
        reserved: 0,
        size: {size},
    }},
    ..
}}"
        );
        assert_eq!(format!("{NOOP_BLOCK:#?}"), expected);
    }

    #[allow(dead_code)]
    fn covariant<'f>(b: GlobalBlock<dyn Fn() + 'static>) -> GlobalBlock<dyn Fn() + 'f> {
        b
    }

