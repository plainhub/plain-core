use crate::http::Method;
use std::{
    fmt,
    fmt::{Debug, Formatter},
};
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MethodFilter(u16);
impl MethodFilter {
    pub const CONNECT: Self = Self::from_bits(0b0_0000_0001);
    pub const DELETE: Self = Self::from_bits(0b0_0000_0010);
    pub const GET: Self = Self::from_bits(0b0_0000_0100);
    pub const HEAD: Self = Self::from_bits(0b0_0000_1000);
    pub const OPTIONS: Self = Self::from_bits(0b0_0001_0000);
    pub const PATCH: Self = Self::from_bits(0b0_0010_0000);
    pub const POST: Self = Self::from_bits(0b0_0100_0000);
    pub const PUT: Self = Self::from_bits(0b0_1000_0000);
    pub const TRACE: Self = Self::from_bits(0b1_0000_0000);
    const fn bits(&self) -> u16 {
        let bits = self;
        bits.0
    }
    const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
    pub(crate) const fn contains(&self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
#[derive(Debug)]
pub struct NoMatchingMethodFilter {
    method: Method,
}
impl NoMatchingMethodFilter {
    pub fn method(&self) -> &Method {
        &self.method
    }
}
impl fmt::Display for NoMatchingMethodFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "no `MethodFilter` for `{}`", self.method.as_str())
    }
}
impl std::error::Error for NoMatchingMethodFilter {}
impl TryFrom<Method> for MethodFilter {
    type Error = NoMatchingMethodFilter;
    fn try_from(m: Method) -> Result<Self, NoMatchingMethodFilter> {
        match m {
            Method::CONNECT => Ok(MethodFilter::CONNECT),
            Method::DELETE => Ok(MethodFilter::DELETE),
            Method::GET => Ok(MethodFilter::GET),
            Method::HEAD => Ok(MethodFilter::HEAD),
            Method::OPTIONS => Ok(MethodFilter::OPTIONS),
            Method::PATCH => Ok(MethodFilter::PATCH),
            Method::POST => Ok(MethodFilter::POST),
            Method::PUT => Ok(MethodFilter::PUT),
            Method::TRACE => Ok(MethodFilter::TRACE),
            other => Err(NoMatchingMethodFilter { method: other }),
        }
    }
}
