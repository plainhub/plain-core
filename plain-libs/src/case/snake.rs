use std::{
    borrow::ToOwned,
    fmt,
    string::{String, ToString},
};

use crate::case::{lowercase, transform};

pub trait ToSnakeCase: ToOwned {
    fn to_snake_case(&self) -> Self::Owned;
}

impl ToSnakeCase for str {
    fn to_snake_case(&self) -> String {
        AsSnakeCase(self).to_string()
    }
}

pub struct AsSnakeCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsSnakeCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), lowercase, |f| write!(f, "_"), f)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/case/snake.rs"]
mod tests;
