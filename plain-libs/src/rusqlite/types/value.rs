use super::{Null, Type};
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
impl From<Null> for Value {
    #[inline]
    fn from(_: Null) -> Self {
        Self::Null
    }
}
impl From<bool> for Value {
    #[inline]
    fn from(i: bool) -> Self {
        Self::Integer(i64::from(i))
    }
}
impl From<isize> for Value {
    #[inline]
    fn from(i: isize) -> Self {
        Self::Integer(i as i64)
    }
}
macro_rules! from_i64 (($t : ty) => (impl From <$t > for Value { #[inline] fn from (i : $t) -> Value { Value::Integer (i64::from (i)) } })) ;
from_i64!(i8);
from_i64!(i16);
from_i64!(i32);
from_i64!(u8);
from_i64!(u16);
from_i64!(u32);
impl From<i64> for Value {
    #[inline]
    fn from(i: i64) -> Self {
        Self::Integer(i)
    }
}
impl From<f32> for Value {
    #[inline]
    fn from(f: f32) -> Self {
        Self::Real(f.into())
    }
}
impl From<f64> for Value {
    #[inline]
    fn from(f: f64) -> Self {
        Self::Real(f)
    }
}
impl From<String> for Value {
    #[inline]
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl From<Vec<u8>> for Value {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        Self::Blob(v)
    }
}
impl<T> From<Option<T>> for Value
where
    T: Into<Self>,
{
    #[inline]
    fn from(v: Option<T>) -> Self {
        match v {
            Some(x) => x.into(),
            None => Self::Null,
        }
    }
}
impl Value {
    #[inline]
    #[must_use]
    pub fn data_type(&self) -> Type {
        match *self {
            Self::Null => Type::Null,
            Self::Integer(_) => Type::Integer,
            Self::Real(_) => Type::Real,
            Self::Text(_) => Type::Text,
            Self::Blob(_) => Type::Blob,
        }
    }
}
