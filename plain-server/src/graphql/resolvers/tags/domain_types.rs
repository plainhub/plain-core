use plain_graphql::{ID, SimpleObject};
use plain_rs::{features::enums::DataType, infrastructure::db::TagRow};

#[derive(SimpleObject, Clone, Debug)]
pub struct Tag {
    pub id: ID,
    pub name: String,
    pub r#type: DataType,
    pub count: i32,
}

impl Tag {
    pub(crate) fn from_row(row: &TagRow) -> Self {
        Self {
            id: row.id.clone().into(),
            name: row.name.clone(),
            r#type: row.kind,
            count: row.count,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct TagRelation {
    #[graphql(name = "tagId")]
    pub tag_id: ID,
    pub key: String,
}
