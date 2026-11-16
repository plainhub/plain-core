use crate::features::enums::DataType;

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
