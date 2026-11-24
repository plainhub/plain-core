use super::{DataType, db, failed};
use crate::graphql::resolvers::tags::domain_types::{Tag, TagRelation};
use plain_graphql::{Context, Object};
use plain_rs::features::tags;

#[derive(Default)]
pub struct TagsQuery;

#[Object]
impl TagsQuery {
    fn tags(&self, ctx: &Context<'_>, r#type: DataType) -> plain_graphql::Result<Vec<Tag>> {
        let db = db(ctx);
        Ok(tags::tags_by_type(db, r#type)
            .map_err(failed)?
            .iter()
            .map(Tag::from_row)
            .collect())
    }

    fn tag_relations(
        &self,
        ctx: &Context<'_>,
        r#type: DataType,
        keys: Vec<String>,
    ) -> plain_graphql::Result<Vec<TagRelation>> {
        let db = db(ctx);
        Ok(tags::relations_for_keys_of_kind(db, &keys, r#type)
            .map_err(failed)?
            .iter()
            .map(|row| TagRelation {
                tag_id: plain_graphql::ID::from(row.tag_id.clone()),
                key: row.target_key.clone(),
            })
            .collect())
    }

    fn tag_keys(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<Vec<String>> {
        let db = db(ctx);
        tags::keys_for_tag(db, id.as_str()).map_err(failed)
    }
}
