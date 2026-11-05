//! Provides a macro, `assert_matches!`, which tests whether a value
//! matches a given pattern, causing a panic if the match fails.
//!
//! See the macro [`assert_matches!`] documentation for more information.
//!
//! Also provides a debug-only counterpart, [`debug_assert_matches!`].
//!
//! See the macro [`debug_assert_matches!`] documentation for more information
//! about this macro.
//!
//! [`assert_matches!`]: macro.assert_matches.html
//! [`debug_assert_matches!`]: macro.debug_assert_matches.html

#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

/// Asserts that an expression matches a given pattern.
///
/// A guard expression may be supplied to add further restrictions to the
/// expected value of the expression.
///
/// A `match` arm may be supplied to perform additional assertions or to yield
/// a value from the macro invocation.
///
/// # Examples
///
/// ```
/// #[macro_use] extern crate assert_matches;
///
/// #[derive(Debug)]
/// enum Foo {
///     A(i32),
///     B(&'static str),
/// }
///
/// # fn main() {
/// let a = Foo::A(1);
///
/// // Assert that `a` matches the pattern `Foo::A(_)`.
/// assert_matches!(a, Foo::A(_));
///
/// // Assert that `a` matches the pattern and
/// // that the contained value meets the condition `i > 0`.
/// assert_matches!(a, Foo::A(i) if i > 0);
///
/// let b = Foo::B("foobar");
///
/// // Assert that `b` matches the pattern `Foo::B(_)`.
/// assert_matches!(b, Foo::B(s) => {
///     // Perform additional assertions on the variable binding `s`.
///     assert!(s.starts_with("foo"));
///     assert!(s.ends_with("bar"));
/// });
///
/// // Assert that `b` matches the pattern and yield the string `s`.
/// let s = assert_matches!(b, Foo::B(s) => s);
///
/// // Perform an assertion on the value `s`.
/// assert_eq!(s, "foobar");
/// # }
/// ```
#[macro_export]
macro_rules! assert_matches {
    ( $e:expr , $($pat:pat)|+ ) => {
        match $e {
            $($pat)|+ => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+))
        }
    };
    ( $e:expr , $($pat:pat)|+ if $cond:expr ) => {
        match $e {
            $($pat)|+ if $cond => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+ if $cond))
        }
    };
    ( $e:expr , $($pat:pat)|+ => $arm:expr ) => {
        match $e {
            $($pat)|+ => $arm,
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+))
        }
    };
    ( $e:expr , $($pat:pat)|+ if $cond:expr => $arm:expr ) => {
        match $e {
            $($pat)|+ if $cond => $arm,
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+ if $cond))
        }
    };
    ( $e:expr , $($pat:pat)|+ , $($arg:tt)* ) => {
        match $e {
            $($pat)|+ => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`: {}",
                e, stringify!($($pat)|+), format_args!($($arg)*))
        }
    };
    ( $e:expr , $($pat:pat)|+ if $cond:expr , $($arg:tt)* ) => {
        match $e {
            $($pat)|+ if $cond => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`: {}",
                e, stringify!($($pat)|+ if $cond), format_args!($($arg)*))
        }
    };
    ( $e:expr , $($pat:pat)|+ => $arm:expr , $($arg:tt)* ) => {
        match $e {
            $($pat)|+ => $arm,
            ref e => panic!("assertion failed: `{:?}` does not match `{}`: {}",
                e, stringify!($($pat)|+), format_args!($($arg)*))
        }
    };
    ( $e:expr , $($pat:pat)|+ if $cond:expr => $arm:expr , $($arg:tt)* ) => {
        match $e {
            $($pat)|+ if $cond => $arm,
            ref e => panic!("assertion failed: `{:?}` does not match `{}`: {}",
                e, stringify!($($pat)|+ if $cond), format_args!($($arg)*))
        }
    };
}

/// Asserts that an expression matches a given pattern.
///
/// Unlike [`assert_matches!`], `debug_assert_matches!` statements are only enabled
/// in non-optimized builds by default. An optimized build will omit all
/// `debug_assert_matches!` statements unless `-C debug-assertions` is passed
/// to the compiler.
///
/// See the macro [`assert_matches!`] documentation for more information.
///
/// [`assert_matches!`]: macro.assert_matches.html
#[macro_export(local_inner_macros)]
macro_rules! debug_assert_matches {
    ( $($tt:tt)* ) => { {
        if _assert_matches_cfg!(debug_assertions) {
            assert_matches!($($tt)*);
        }
    } }
}

#[doc(hidden)]
#[macro_export]
macro_rules! _assert_matches_cfg {
    ( $($tt:tt)* ) => { cfg!($($tt)*) }
}

#[cfg(test)]
#[path = "../tests/unit/src/lib_test.rs"]
mod test;
