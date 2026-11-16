use crate::features::enums::DataType;
use plain_libs::rusqlite::types::{
    FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef,
};
use serde::Deserialize;
use serde::de::value::StrDeserializer;

impl ToSql for DataType {
    fn to_sql(&self) -> plain_libs::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.storage_name().as_bytes(),
        )))
    }
}

impl FromSql for DataType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::deserialize(StrDeserializer::<serde::de::value::Error>::new(
            value.as_str()?,
        ))
        .map_err(|error| FromSqlError::Other(Box::new(error)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub kind: DataType,
    pub count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TagRelationRow {
    pub tag_id: String,
    pub target_key: String,
}
