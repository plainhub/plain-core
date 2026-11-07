#[macro_export(local_inner_macros)]
macro_rules! assert_impl {
    (for($($generic:tt)*) $ty:ty: $($rest:tt)*) => {
        const _: () = {
            fn assert_impl<$($generic)*>() {
                // Construct an expression using `True`/`False` and their
                // operators, that corresponds to the provided expression.
                let _: $crate::assertions::True = $crate::assertions::_does_impl!($ty: $($rest)*);
            }
        };
    };
    ($ty:ty: $($rest:tt)*) => {
        // Construct an expression using `True`/`False` and their operators,
        // that corresponds to the provided expression.
        const _: $crate::assertions::True = $crate::assertions::_does_impl!($ty: $($rest)*);
    };
}
