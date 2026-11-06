#![cfg_attr(
    docsrs,
    doc = "This is a stub. The latest API documentation is here: <https://microsoft.github.io/windows-docs-rs/>"
)]
#![cfg_attr(docsrs, doc = "")]
#![doc = "\nLearn more about Rust for Windows here: <https://github.com/microsoft/windows-rs>\n\n[Feature search](https://microsoft.github.io/windows-rs/features/#/0.57.0)\n"]
#![doc(html_no_source)]
#![allow(
    non_snake_case,
    clashing_extern_declarations,
    non_upper_case_globals,
    non_camel_case_types,
    missing_docs,
    clippy::all
)]
#![cfg_attr(not(any()), doc(hidden))]
#[allow(unused_extern_crates)]
extern crate self as windows;
pub use windows_core as core;
include!("Windows/mod.rs");
