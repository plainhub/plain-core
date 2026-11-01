use crate::infrastructure::db::Db;
use crate::infrastructure::db::TagRelationRow;
use crate::infrastructure::db::TagRow;
use crate::infrastructure::db::tag;
use crate::infrastructure::error::DomainResult;

pub fn create_tag(db: &Db, kind: i32, name: &str) -> DomainResult<TagRow> {
    let row = TagRow {
        id: plain_libs::utils::shortid::new_id(),
        name: name.into(),
        kind,
        count: 0,
    };
    tag::insert_tag(db, &row)?;
    Ok(row)
}
pub fn update_tag(db: &Db, id: &str, name: &str) -> DomainResult<Option<TagRow>> {
    Ok(tag::rename_tag(db, id, name)?)
}
pub fn delete_tag(db: &Db, id: &str) -> DomainResult<()> {
    Ok(tag::delete_tag(db, id)?)
}
pub fn tag_by_id(db: &Db, id: &str) -> DomainResult<Option<TagRow>> {
    Ok(tag::tag_by_id(db, id)?)
}
pub fn tags_by_type(db: &Db, kind: i32) -> DomainResult<Vec<TagRow>> {
    Ok(tag::tags_by_type(db, kind)?)
}
pub fn relations_for_keys_of_kind(
    db: &Db,
    keys: &[String],
    kind: i32,
) -> DomainResult<Vec<TagRelationRow>> {
    Ok(tag::relations_for_keys_of_kind(db, keys, kind)?)
}
pub fn relations_for_key(db: &Db, key: &str) -> DomainResult<Vec<TagRelationRow>> {
    Ok(tag::relations_for_key(db, key)?)
}
pub fn tags_for_key_of_kind(db: &Db, key: &str, kind: i32) -> DomainResult<Vec<TagRow>> {
    Ok(tag::tags_for_key_of_kind(db, key, kind)?)
}
pub fn keys_for_tag(db: &Db, id: &str) -> DomainResult<Vec<String>> {
    Ok(tag::keys_for_tag(db, id)?)
}
pub fn add_relations(db: &Db, relations: &[(String, String)]) -> DomainResult<()> {
    Ok(tag::insert_relations(db, relations)?)
}
pub fn remove_relations(db: &Db, keys: &[String], ids: &[String]) -> DomainResult<()> {
    Ok(tag::remove_relations(db, keys, ids)?)
}
pub fn remove_relations_for_keys(db: &Db, keys: &[String]) -> DomainResult<()> {
    Ok(tag::remove_relations_for_keys(db, keys)?)
}
pub fn add_relations_checked(db: &Db, relations: &[(String, String)]) -> DomainResult<()> {
    add_relations(db, relations)
}
pub fn edit_relations(
    db: &Db,
    kind: i32,
    key: &str,
    add: &[String],
    remove: &[String],
) -> DomainResult<()> {
    update_relations(db, kind, &[key.into()], add, remove)
}

mod relations;
pub use relations::update_relations;

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/tags/mod.rs"]
mod tests;
