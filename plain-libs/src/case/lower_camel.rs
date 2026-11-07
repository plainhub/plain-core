use core::fmt;

use std::{
    borrow::ToOwned,
    string::{String, ToString},
};

use crate::case::{capitalize, lowercase, transform};

pub trait ToLowerCamelCase: ToOwned {
    fn to_lower_camel_case(&self) -> Self::Owned;
}

impl ToLowerCamelCase for str {
    fn to_lower_camel_case(&self) -> String {
        AsLowerCamelCase(self).to_string()
    }
}

pub struct AsLowerCamelCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsLowerCamelCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        transform(
            self.0.as_ref(),
            |s, f| {
                if first {
                    first = false;
                    lowercase(s, f)
                } else {
                    capitalize(s, f)
                }
            },
            |_| Ok(()),
            f,
        )
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/case/lower_camel.rs"]
mod tests;
