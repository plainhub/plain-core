
    #[allow(dead_code)]
    struct TISInputSource {}

    // Test old syntax still works (used by dependencies).
    cf_type!(
        #[encoding_name = "__TISInputSource"]
        unsafe impl TISInputSource {}
    );

