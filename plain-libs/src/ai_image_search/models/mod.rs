pub mod manifest;

pub mod package;

#[cfg(any(test, feature = "test-seams"))]
#[doc(hidden)]
pub mod test_fixture;
