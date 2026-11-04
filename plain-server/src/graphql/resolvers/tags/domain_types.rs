use plain_graphql::{ID, SimpleObject};

#[derive(SimpleObject, Clone, Debug)]
pub struct Tag {
    pub id: ID,
    pub name: String,
    pub r#type: i32,
    pub count: i32,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct TagRelation {
    #[graphql(name = "tagId")]
    pub tag_id: ID,
    pub key: String,
}
