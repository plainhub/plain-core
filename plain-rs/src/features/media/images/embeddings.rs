use crate::infrastructure::db::Db;
use crate::infrastructure::error::DomainError;
use crate::infrastructure::error::DomainResult;
use plain_libs::rusqlite::params;
pub struct EmbeddingInput {
    pub id: String,
    pub path: String,
    pub embedding: Vec<f32>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub image_id: String,
    pub score: f32,
}
fn vector(bytes: &[u8]) -> DomainResult<Vec<f32>> {
    plain_libs::utils::vector::decode_f32_be(bytes).map_err(DomainError::Other)
}
fn decode(value: &str) -> DomainResult<Vec<u8>> {
    plain_libs::utils::base64::base64_decode_checked(value)
        .map_err(|e| DomainError::Other(e.to_string()))
}
pub fn save(db: &Db, items: &[EmbeddingInput]) -> DomainResult<()> {
    if items.len() > 100 {
        return Err(DomainError::Other("embedding batch too large".into()));
    }
    for item in items {
        if item.id.is_empty() || item.path.is_empty() {
            return Err(DomainError::Other("missing embedding identity".into()));
        }
        if item.embedding.is_empty() || item.embedding.len() > 4096 {
            return Err(DomainError::Other("invalid embedding dimensions".into()));
        }
        if item.embedding.iter().any(|value| !value.is_finite()) {
            return Err(DomainError::Other("invalid embedding values".into()));
        }
    }
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut insert = tx.prepare("INSERT INTO image_embeddings (file_id,path,embedding) VALUES (?1,?2,?3) ON CONFLICT(file_id) DO UPDATE SET path=excluded.path,embedding=excluded.embedding")?;
            let mut blob = Vec::new();
            for item in items {
                blob.clear();
                blob.extend(item.embedding.iter().flat_map(|value| value.to_be_bytes()));
                insert.execute(params![item.id, item.path, &blob])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}
pub fn search(db: &Db, query: &str, limit: usize) -> DomainResult<Vec<SearchResult>> {
    if limit > 500 {
        return Err(DomainError::Other("image search limit too large".into()));
    }
    if limit == 0 {
        vector(&decode(query)?)?;
        return Ok(Vec::new());
    }
    let mut results = search_with_score(db, query, 0.15)?;
    results.truncate(limit);
    Ok(results)
}
pub fn search_with_score(
    db: &Db,
    query: &str,
    minimum_score: f32,
) -> DomainResult<Vec<SearchResult>> {
    if !minimum_score.is_finite() || !(-1.0..=1.0).contains(&minimum_score) {
        return Err(DomainError::Other("Invalid model score threshold".into()));
    }
    let query = vector(&decode(query)?)?;
    db.with_conn(|c| {
        let mut statement =
            c.prepare("SELECT file_id,embedding FROM image_embeddings ORDER BY file_id")?;
        let mut rows = statement.query([])?;
        let mut matches = Vec::<SearchResult>::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let values = vector(&row.get::<_, Vec<u8>>(1)?)?;
            if values.len() != query.len() {
                return Err(DomainError::Other("embedding dimension mismatch".into()));
            }
            let score = query
                .iter()
                .zip(&values)
                .fold(0.0_f32, |sum, (a, b)| sum + a * b);
            if !score.is_finite() {
                return Err(DomainError::Other("invalid image search score".into()));
            }
            if score < minimum_score {
                continue;
            }
            matches.push(SearchResult {
                image_id: id,
                score,
            });
        }
        matches.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.image_id.cmp(&b.image_id))
        });
        Ok(matches)
    })
}
pub fn ids(db: &Db) -> DomainResult<Vec<String>> {
    Ok(db.embedding_ids()?)
}
pub fn count(db: &Db) -> DomainResult<i64> {
    Ok(db.embedding_count()?)
}
pub fn delete(db: &Db, ids: &[String]) -> DomainResult<usize> {
    Ok(db.embedding_delete_by_ids(ids)?)
}
pub fn clear(db: &Db) -> DomainResult<usize> {
    Ok(db.embedding_delete_all()?)
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/media/images/embeddings.rs"]
mod tests;
