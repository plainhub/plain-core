//! # Strum
//!
//! Strum is a set of macros and traits for working with
//! enums and strings easier in Rust.
//!
//! This crate only contains derive macros for use with the
//! [`strum`](https://docs.rs/strum)
//! crate.  The macros provied by this crate are also available by
//! enabling the `derive` feature in aforementioned `strum` crate.

#![recursion_limit = "128"]

extern crate proc_macro;

mod helpers;
mod macros;

use proc_macro2::TokenStream;
use std::env;
use syn::DeriveInput;

fn debug_print_generated(ast: &DeriveInput, toks: &TokenStream) {
    let debug = env::var("STRUM_DEBUG");
    if let Ok(s) = debug {
        if s == "1" {
            println!("{}", toks);
        }

        if ast.ident == s {
            println!("{}", toks);
        }
    }
}

#[proc_macro_derive(FromRepr, attributes(strum))]
pub fn from_repr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::from_repr::from_repr_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

#[proc_macro_derive(EnumCount, attributes(strum))]
pub fn enum_count(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);
    let toks =
        macros::enum_count::enum_count_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}
