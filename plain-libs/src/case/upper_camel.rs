use core::fmt;

use std::{
    borrow::ToOwned,
    string::{String, ToString},
};

use crate::case::{capitalize, transform};

pub trait ToUpperCamelCase: ToOwned {
    fn to_upper_camel_case(&self) -> Self::Owned;
}

impl ToUpperCamelCase for str {
    fn to_upper_camel_case(&self) -> String {
        AsUpperCamelCase(self).to_string()
    }
}

pub trait ToPascalCase: ToOwned {
    fn to_pascal_case(&self) -> Self::Owned;
}

impl<T: ?Sized + ToUpperCamelCase> ToPascalCase for T {
    fn to_pascal_case(&self) -> Self::Owned {
        self.to_upper_camel_case()
    }
}

pub struct AsUpperCamelCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsUpperCamelCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), capitalize, |_| Ok(()), f)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/case/upper_camel.rs"]
mod tests;
