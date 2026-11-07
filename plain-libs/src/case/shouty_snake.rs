use core::fmt;

use std::{borrow::ToOwned, string::ToString};

use crate::case::{transform, uppercase};

pub trait ToShoutySnakeCase: ToOwned {
    fn to_shouty_snake_case(&self) -> Self::Owned;
}

impl ToShoutySnakeCase for str {
    fn to_shouty_snake_case(&self) -> Self::Owned {
        AsShoutySnakeCase(self).to_string()
    }
}

pub struct AsShoutySnakeCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsShoutySnakeCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), uppercase, |f| write!(f, "_"), f)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/case/shouty_snake.rs"]
mod tests;
