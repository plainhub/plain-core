pub use self::from_sql::{FromSql, FromSqlError, FromSqlResult};
pub use self::to_sql::{ToSql, ToSqlOutput};
pub use self::value::Value;
pub use self::value_ref::ValueRef;
use std::fmt;
mod from_sql;
mod to_sql;
mod value;
mod value_ref;
#[derive(Copy, Clone)]
pub struct Null;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Null,
    Integer,
    Real,
    Text,
    Blob,
}
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Null => f.pad("Null"),
            Self::Integer => f.pad("Integer"),
            Self::Real => f.pad("Real"),
            Self::Text => f.pad("Text"),
            Self::Blob => f.pad("Blob"),
        }
    }
}
