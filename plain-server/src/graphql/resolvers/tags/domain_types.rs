use async_graphql::ID;
use plain_graphql_macros::SimpleObject;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(getters = false)]
pub struct Tag {
    pub id: ID,
    pub name: String,
    pub r#type: i32,
    pub count: i32,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(getters = false)]
pub struct TagRelation {
    #[graphql(name = "tagId")]
    pub tag_id: ID,
    pub key: String,
}
