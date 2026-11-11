use super::{Value, ValueRef};
use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::str::Utf8Error;
#[derive(Debug)]
#[non_exhaustive]
pub enum FromSqlError {
    InvalidType,
    OutOfRange(i64),
    Utf8Error(Utf8Error),
    InvalidBlobSize {
        expected_size: usize,
        blob_size: usize,
    },
    Other(Box<dyn Error + Send + Sync + 'static>),
}
impl FromSqlError {
    pub fn other<E: Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Other(Box::new(error))
    }
}
impl PartialEq for FromSqlError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidType, Self::InvalidType) => true,
            (Self::OutOfRange(n1), Self::OutOfRange(n2)) => n1 == n2,
            (Self::Utf8Error(u1), Self::Utf8Error(u2)) => u1 == u2,
            (
                Self::InvalidBlobSize {
                    expected_size: es1,
                    blob_size: bs1,
                },
                Self::InvalidBlobSize {
                    expected_size: es2,
                    blob_size: bs2,
                },
            ) => es1 == es2 && bs1 == bs2,
            (..) => false,
        }
    }
}
impl fmt::Display for FromSqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::InvalidType => write!(f, "Invalid type"),
            Self::OutOfRange(i) => write!(f, "Value {i} out of range"),
            Self::Utf8Error(ref err) => err.fmt(f),
            Self::InvalidBlobSize {
                expected_size,
                blob_size,
            } => {
                write!(
                    f,
                    "Cannot read {expected_size} byte value out of {blob_size} byte blob"
                )
            }
            Self::Other(ref err) => err.fmt(f),
        }
    }
}
impl Error for FromSqlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Utf8Error(err) => Some(err),
            Self::Other(err) => Some(&**err),
            _ => None,
        }
    }
}
impl From<Utf8Error> for FromSqlError {
    #[cold]
    fn from(err: Utf8Error) -> Self {
        Self::Utf8Error(err)
    }
}
pub type FromSqlResult<T> = Result<T, FromSqlError>;
pub trait FromSql: Sized {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self>;
}
macro_rules! from_sql_integral (($t : ident) => (impl FromSql for $t { #[inline] fn column_result (value : ValueRef <'_ >) -> FromSqlResult < Self > { let i = i64::column_result (value) ?; i . try_into () . map_err (| _ | FromSqlError::OutOfRange (i)) } }) ; (non_zero $nz : ty , $z : ty) => (impl FromSql for $nz { #[inline] fn column_result (value : ValueRef <'_ >) -> FromSqlResult < Self > { let i = <$z >::column_result (value) ?; <$nz >::new (i) . ok_or (FromSqlError::OutOfRange (0)) } })) ;
from_sql_integral!(i8);
from_sql_integral!(i16);
from_sql_integral!(i32);
from_sql_integral!(isize);
from_sql_integral!(u8);
from_sql_integral!(u16);
from_sql_integral!(u32);
from_sql_integral ! (non_zero std::num::NonZeroIsize , isize);
from_sql_integral ! (non_zero std::num::NonZeroI8 , i8);
from_sql_integral ! (non_zero std::num::NonZeroI16 , i16);
from_sql_integral ! (non_zero std::num::NonZeroI32 , i32);
from_sql_integral ! (non_zero std::num::NonZeroI64 , i64);
from_sql_integral ! (non_zero std::num::NonZeroU8 , u8);
from_sql_integral ! (non_zero std::num::NonZeroU16 , u16);
from_sql_integral ! (non_zero std::num::NonZeroU32 , u32);
impl FromSql for i64 {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_i64()
    }
}
impl FromSql for f32 {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Integer(i) => Ok(i as Self),
            ValueRef::Real(f) => Ok(f as Self),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}
impl FromSql for f64 {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Integer(i) => Ok(i as Self),
            ValueRef::Real(f) => Ok(f),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}
impl FromSql for bool {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(|i| i != 0)
    }
}
impl FromSql for String {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(ToString::to_string)
    }
}
impl FromSql for Box<str> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(Into::into)
    }
}
impl FromSql for std::rc::Rc<str> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(Into::into)
    }
}
impl FromSql for std::sync::Arc<str> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(Into::into)
    }
}
impl FromSql for Vec<u8> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_blob().map(<[u8]>::to_vec)
    }
}
impl FromSql for Box<[u8]> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_blob().map(Box::<[u8]>::from)
    }
}
impl FromSql for std::rc::Rc<[u8]> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_blob().map(std::rc::Rc::<[u8]>::from)
    }
}
impl FromSql for std::sync::Arc<[u8]> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_blob().map(std::sync::Arc::<[u8]>::from)
    }
}
impl<const N: usize> FromSql for [u8; N] {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let slice = value.as_blob()?;
        slice.try_into().map_err(|_| FromSqlError::InvalidBlobSize {
            expected_size: N,
            blob_size: slice.len(),
        })
    }
}
impl<T: FromSql> FromSql for Option<T> {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Null => Ok(None),
            _ => FromSql::column_result(value).map(Some),
        }
    }
}
impl<T: ?Sized> FromSql for Cow<'_, T>
where
    T: ToOwned,
    T::Owned: FromSql,
{
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        <T::Owned>::column_result(value).map(Cow::Owned)
    }
}
impl FromSql for Value {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.try_into()
    }
}
