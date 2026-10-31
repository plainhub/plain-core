pub mod features;
pub mod libs;

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(any(test, feature = "test-seams"))]
#[path = "../../tests/plain-rs/support/tempdirs.rs"]
pub mod test_tempdirs;

#[cfg(all(test, feature = "runtime"))]
#[path = "../../tests/plain-rs/support/runtime.rs"]
pub(crate) mod test_runtime;

#[cfg(all(test, feature = "runtime"))]
#[path = "../../tests/plain-rs/support/host.rs"]
pub(crate) mod test_support;

#[cfg(all(feature = "runtime", any(test, feature = "test-seams")))]
#[path = "../../tests/plain-rs/support/apk.rs"]
pub mod test_apk;
