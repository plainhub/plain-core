#![forbid(unsafe_code)]

mod args;
mod complex_object;
mod r#enum;
mod input_object;
mod interface;
mod merged_subscription;
mod meta;
mod scalar;
mod subscription;
mod union;

mod merged_object;
mod object;
mod output_type;
mod simple_object;
mod utils;

use crate::meta::FromMeta;
use proc_macro::TokenStream;
use syn::{DeriveInput, ItemImpl, parse_macro_input};

#[proc_macro_attribute]
pub fn object(args: TokenStream, input: TokenStream) -> TokenStream {
    let nested = match meta::parse_list(args.into()) {
        Ok(args) => args,
        Err(error) => return error.to_compile_error().into(),
    };
    let args = match args::Object::from_list(nested) {
        Ok(args) => args,
        Err(error) => return error.to_compile_error().into(),
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
            Err(error) => return error.to_compile_error().into(),
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
            Err(error) => return error.to_compile_error().into(),
        };
    match merged_object::generate(&args) {
        Ok(output) => output.into(),
        Err(error) => error.write_errors().into(),
    }
}

macro_rules! derive_type {
    ($entry:ident, $options:ident, $generator:ident) => {
        #[proc_macro_derive($options, attributes(graphql))]
        pub fn $entry(input: TokenStream) -> TokenStream {
            let args = match args::$options::from_derive_input(&parse_macro_input!(
                input as DeriveInput
            )) {
                Ok(args) => args,
                Err(error) => return error.to_compile_error().into(),
            };
            match $generator::generate(&args) {
                Ok(output) => output.into(),
                Err(error) => error.write_errors().into(),
            }
        }
    };
}
derive_type!(enum_type, Enum, r#enum);
derive_type!(input_object, InputObject, input_object);
derive_type!(interface_type, Interface, interface);
derive_type!(union_type, Union, union);
derive_type!(merged_subscription, MergedSubscription, merged_subscription);
macro_rules! attribute_type {
    ($entry:ident, $options:ident, $generator:ident) => {
        #[proc_macro_attribute]
        pub fn $entry(args: TokenStream, input: TokenStream) -> TokenStream {
            let nested = match meta::parse_list(args.into()) {
                Ok(args) => args,
                Err(error) => return error.to_compile_error().into(),
            };
            let args = match args::$options::from_list(nested) {
                Ok(args) => args,
                Err(error) => return error.to_compile_error().into(),
            };
            match $generator::generate(&args, &mut parse_macro_input!(input as ItemImpl)) {
                Ok(output) => output.into(),
                Err(error) => error.write_errors().into(),
            }
        }
    };
}
attribute_type!(complex_object, ComplexObject, complex_object);
attribute_type!(scalar, Scalar, scalar);
attribute_type!(subscription, Subscription, subscription);

#[path = "../../../plain-libs/src/case/mod.rs"]
mod case;
