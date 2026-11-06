//! Various explanations and topics of discussion.
#![allow(clippy::needless_doctest_main)]

pub mod about_generated;

pub mod alternatives {}
#[cfg(not(any()))]
pub mod crate_interop {}
pub mod cross_compiling {}
pub mod kvo {}
pub mod layered_safety {}
pub mod migrating_from_objc {}
pub mod mvc {}
pub mod interior_mutability {}
pub mod swift {}
pub mod weak_property {} // Referenced by header-translator
#[cfg(not(any()))]
pub mod run_loop {}

#[cfg(not(doctest))]
pub mod changelog {}
