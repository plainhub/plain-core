#![forbid(unsafe_code)]

mod args;
mod merged_object;
mod object;
mod output_type;
mod simple_object;
mod utils;
mod validators;

use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use syn::{DeriveInput, ItemImpl, parse_macro_input};

#[proc_macro_attribute]
pub fn object(args: TokenStream, input: TokenStream) -> TokenStream {
    let nested = match darling::ast::NestedMeta::parse_meta_list(args.into()) {
        Ok(args) => args,
        Err(error) => return darling::Error::from(error).write_errors().into(),
    };
    let args = match args::Object::from_list(&nested) {
        Ok(args) => args,
        Err(error) => return error.write_errors().into(),
    };
    match object::generate(&args, &mut parse_macro_input!(input as ItemImpl)) {
        Ok(output) => output.into(),
        Err(error) => error.write_errors().into(),
    }
}

#[proc_macro_derive(SimpleObject, attributes(graphql))]
pub fn simple_object(input: TokenStream) -> TokenStream {
    let args =
        match args::SimpleObject::from_derive_input(&parse_macro_input!(input as DeriveInput)) {
            Ok(args) => args,
            Err(error) => return error.write_errors().into(),
        };
    match simple_object::generate(&args) {
        Ok(output) => output.into(),
        Err(error) => error.write_errors().into(),
    }
}

#[proc_macro_derive(MergedObject, attributes(graphql))]
pub fn merged_object(input: TokenStream) -> TokenStream {
    let args =
        match args::MergedObject::from_derive_input(&parse_macro_input!(input as DeriveInput)) {
            Ok(args) => args,
            Err(error) => return error.write_errors().into(),
        };
    match merged_object::generate(&args) {
        Ok(output) => output.into(),
        Err(error) => error.write_errors().into(),
    }
}
