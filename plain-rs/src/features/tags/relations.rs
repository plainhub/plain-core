use crate::libs::db::{Db, tag};
use crate::libs::error::{DomainError, DomainResult};

pub fn update_relations(
    db: &Db,
    kind: i32,
    keys: &[String],
    add: &[String],
    remove: &[String],
) -> DomainResult<()> {
    if keys.iter().any(|key| key.trim().is_empty()) {
        return Err(DomainError::Other("empty tag key".into()));
    }
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        for id in add.iter().chain(remove) {
            if tag::io::tag_by_id(&tx, id)?.is_none_or(|row| row.kind != kind) {
                return Err(DomainError::Other("tag type mismatch".into()));
            }
        }
        let relations = add
            .iter()
            .flat_map(|id| keys.iter().map(move |key| (id.clone(), key.clone())))
            .collect::<Vec<_>>();
        tag::io::insert_relations(&tx, &relations)?;
        tag::io::remove_relations(&tx, keys, remove)?;
        tx.commit()?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/tags/relations.rs"]
mod tests;
