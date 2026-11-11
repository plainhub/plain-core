use super::{Type, Value};
use crate::rusqlite::types::{FromSqlError, FromSqlResult};
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a [u8]),
    Blob(&'a [u8]),
}
impl ValueRef<'_> {
    #[inline]
    #[must_use]
    pub fn data_type(&self) -> Type {
        match *self {
            ValueRef::Null => Type::Null,
            ValueRef::Integer(_) => Type::Integer,
            ValueRef::Real(_) => Type::Real,
            ValueRef::Text(_) => Type::Text,
            ValueRef::Blob(_) => Type::Blob,
        }
    }
}
impl<'a> ValueRef<'a> {
    #[inline]
    pub fn as_i64(&self) -> FromSqlResult<i64> {
        match *self {
            ValueRef::Integer(i) => Ok(i),
            _ => Err(FromSqlError::InvalidType),
        }
    }

    #[inline]
    pub fn as_f64(&self) -> FromSqlResult<f64> {
        match *self {
            ValueRef::Real(f) => Ok(f),
            _ => Err(FromSqlError::InvalidType),
        }
    }

    #[inline]
    pub fn as_str(&self) -> FromSqlResult<&'a str> {
        match *self {
            ValueRef::Text(t) => Ok(std::str::from_utf8(t)?),
            _ => Err(FromSqlError::InvalidType),
        }
    }

    #[inline]
    pub fn as_blob(&self) -> FromSqlResult<&'a [u8]> {
        match *self {
            ValueRef::Blob(b) => Ok(b),
            _ => Err(FromSqlError::InvalidType),
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> FromSqlResult<&'a [u8]> {
        match self {
            ValueRef::Text(s) | ValueRef::Blob(s) => Ok(s),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}
impl TryFrom<ValueRef<'_>> for Value {
    type Error = FromSqlError;
    #[inline]
    #[track_caller]
    fn try_from(borrowed: ValueRef<'_>) -> Result<Self, Self::Error> {
        Ok(match borrowed {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(i) => Self::Integer(i),
            ValueRef::Real(r) => Self::Real(r),
            ValueRef::Text(s) => Self::Text(std::str::from_utf8(s)?.to_owned()),
            ValueRef::Blob(b) => Self::Blob(b.to_vec()),
        })
    }
}
impl<'a> From<&'a str> for ValueRef<'a> {
    #[inline]
    fn from(s: &str) -> ValueRef<'_> {
        ValueRef::Text(s.as_bytes())
    }
}
impl<'a> From<&'a [u8]> for ValueRef<'a> {
    #[inline]
    fn from(s: &[u8]) -> ValueRef<'_> {
        ValueRef::Blob(s)
    }
}
impl<'a> From<&'a Value> for ValueRef<'a> {
    #[inline]
    fn from(value: &'a Value) -> Self {
        match *value {
            Value::Null => ValueRef::Null,
            Value::Integer(i) => ValueRef::Integer(i),
            Value::Real(r) => ValueRef::Real(r),
            Value::Text(ref s) => ValueRef::Text(s.as_bytes()),
            Value::Blob(ref b) => ValueRef::Blob(b),
        }
    }
}
impl<T> From<Option<T>> for ValueRef<'_>
where
    T: Into<Self>,
{
    #[inline]
    fn from(s: Option<T>) -> Self {
        match s {
            Some(x) => x.into(),
            None => ValueRef::Null,
        }
    }
}
