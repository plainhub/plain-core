use super::{Null, Value, ValueRef};
use crate::rusqlite::Result;
use std::borrow::Cow;
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ToSqlOutput<'a> {
    Borrowed(ValueRef<'a>),
    Owned(Value),
}
impl<'a, T: ?Sized> From<&'a T> for ToSqlOutput<'a>
where
    &'a T: Into<ValueRef<'a>>,
{
    #[inline]
    fn from(t: &'a T) -> Self {
        ToSqlOutput::Borrowed(t.into())
    }
}
macro_rules! from_value (($t : ty) => (impl From <$t > for ToSqlOutput <'_ > { #[inline] fn from (t : $t) -> Self { ToSqlOutput::Owned (t . into ()) } }) ; (non_zero $t : ty) => (impl From <$t > for ToSqlOutput <'_ > { #[inline] fn from (t : $t) -> Self { ToSqlOutput::Owned (t . get () . into ()) } })) ;
from_value!(String);
from_value!(Null);
from_value!(bool);
from_value!(i8);
from_value!(i16);
from_value!(i32);
from_value!(i64);
from_value!(isize);
from_value!(u8);
from_value!(u16);
from_value!(u32);
from_value!(f32);
from_value!(f64);
from_value!(Vec<u8>);
from_value ! (non_zero std::num::NonZeroI8);
from_value ! (non_zero std::num::NonZeroI16);
from_value ! (non_zero std::num::NonZeroI32);
from_value ! (non_zero std::num::NonZeroI64);
from_value ! (non_zero std::num::NonZeroIsize);
from_value ! (non_zero std::num::NonZeroU8);
from_value ! (non_zero std::num::NonZeroU16);
from_value ! (non_zero std::num::NonZeroU32);
impl ToSql for ToSqlOutput<'_> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(match *self {
            ToSqlOutput::Borrowed(v) => ToSqlOutput::Borrowed(v),
            ToSqlOutput::Owned(ref v) => ToSqlOutput::Borrowed(ValueRef::from(v)),
        })
    }
}
pub trait ToSql {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>>;
}
impl<T: ToSql + ToOwned + ?Sized> ToSql for Cow<'_, T> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        self.as_ref().to_sql()
    }
}
impl<T: ToSql + ?Sized> ToSql for Box<T> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        self.as_ref().to_sql()
    }
}
impl<T: ToSql + ?Sized> ToSql for std::rc::Rc<T> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        self.as_ref().to_sql()
    }
}
impl<T: ToSql + ?Sized> ToSql for std::sync::Arc<T> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        self.as_ref().to_sql()
    }
}
macro_rules! to_sql_self (($t : ty) => (impl ToSql for $t { #[inline] fn to_sql (& self) -> Result < ToSqlOutput <'_ >> { Ok (ToSqlOutput::from (* self)) } })) ;
to_sql_self!(Null);
to_sql_self!(bool);
to_sql_self!(i8);
to_sql_self!(i16);
to_sql_self!(i32);
to_sql_self!(i64);
to_sql_self!(isize);
to_sql_self!(u8);
to_sql_self!(u16);
to_sql_self!(u32);
to_sql_self!(f32);
to_sql_self!(f64);
to_sql_self!(std::num::NonZeroI8);
to_sql_self!(std::num::NonZeroI16);
to_sql_self!(std::num::NonZeroI32);
to_sql_self!(std::num::NonZeroI64);
to_sql_self!(std::num::NonZeroIsize);
to_sql_self!(std::num::NonZeroU8);
to_sql_self!(std::num::NonZeroU16);
to_sql_self!(std::num::NonZeroU32);
impl<T: ?Sized> ToSql for &'_ T
where
    T: ToSql,
{
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        (*self).to_sql()
    }
}
impl ToSql for String {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}
impl ToSql for str {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self))
    }
}
impl ToSql for Vec<u8> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_slice()))
    }
}
impl<const N: usize> ToSql for [u8; N] {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(&self[..]))
    }
}
impl ToSql for [u8] {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self))
    }
}
impl ToSql for Value {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self))
    }
}
impl<T: ToSql> ToSql for Option<T> {
    #[inline]
    fn to_sql(&self) -> Result<ToSqlOutput<'_>> {
        match *self {
            None => Ok(ToSqlOutput::from(Null)),
            Some(ref t) => t.to_sql(),
        }
    }
}
