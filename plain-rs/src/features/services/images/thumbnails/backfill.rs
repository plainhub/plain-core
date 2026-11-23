use super::{CoreRuntime, Request};
use crate::infrastructure::db::Db;
use plain_libs::rusqlite::{Result as SqlResult, params};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const BATCH_SIZE: usize = 64;

struct Candidate {
    id: String,
    path: String,
    bucket: String,
    modified: String,
    size: i64,
}

pub(super) async fn run(state: CoreRuntime) {
    let mut stop = state.stop.clone();
    loop {
        tokio::select! {
            biased;
            _ = stop.wait_for(|value| *value) => return,
            _ = state.thumbnails.backfill_ready.notified() => {}
        }
        let result = tokio::select! {
            biased;
            _ = stop.wait_for(|value| *value) => return,
            result = populate(&state) => result,
        };
        if let Err(error) = result {
            log::error!("Thumbnail hash backfill failed: {error}");
        }
    }
}

async fn populate(state: &CoreRuntime) -> plain_libs::anyhow::Result<()> {
    let mut pending = VecDeque::from([("VIDEO", None), ("IMAGE", None)]);
    while let Some((kind, cursor)) = pending.pop_front() {
        let db = state.db.clone();
        let batch =
            tokio::task::spawn_blocking(move || read_batch(&db, kind, cursor.as_ref())).await??;
        if batch.len() == BATCH_SIZE {
            let last = batch.last().unwrap();
            pending.push_back((kind, Some((last.modified.clone(), last.id.clone()))));
        }
        let batch = prioritize_covers(batch);
        let mut results = Vec::with_capacity(batch.len());
        let mut decode_errors = 0;
        for candidate in batch {
            match generate(state, &candidate, kind == "VIDEO").await {
                Ok(Some(hash)) => results.push((candidate, hash)),
                Ok(None) => {}
                Err(_) => decode_errors += 1,
            }
        }
        if decode_errors != 0 {
            log::warn!("Unable to generate {decode_errors} thumbnail hashes in a {kind} batch");
        }
        if !results.is_empty() {
            let db = state.db.clone();
            let updated = tokio::task::spawn_blocking(move || write_batch(&db, &results)).await??;
            if updated > 0 {
                let data_type = if kind == "VIDEO" {
                    crate::features::enums::DataType::Video
                } else {
                    crate::features::enums::DataType::Image
                };
                state.media_scans.publish_catalog_types([data_type.kind()]);
            }
        }
    }
    Ok(())
}

fn read_batch(db: &Db, kind: &str, cursor: Option<&(String, String)>) -> SqlResult<Vec<Candidate>> {
    let (modified, id) = cursor.map_or(("9999", ""), |(modified, id)| {
        (modified.as_str(), id.as_str())
    });
    db.with_conn(|conn| {
        let mut statement = conn.prepare_cached(
            "SELECT id,path,bucket_id,updated_at,size_bytes FROM file_catalog
             WHERE kind=?1 AND missing=0 AND trashed_at IS NULL AND indexed=1 AND is_available=1
             AND thumb_hash IS NULL AND kind IN ('IMAGE','VIDEO') AND (updated_at,id)<(?2,?3) AND size_bytes>0
             ORDER BY updated_at DESC,id DESC LIMIT ?4",
        )?;
        statement.query_map(params![kind, modified, id, BATCH_SIZE as i64], |row| {
            Ok(Candidate {
                id: row.get(0)?,
                path: row.get(1)?,
                bucket: row.get(2)?,
                modified: row.get(3)?,
                size: row.get(4)?,
            })
        })?.collect()
    })
}

fn prioritize_covers(batch: Vec<Candidate>) -> Vec<Candidate> {
    let mut covers = Vec::with_capacity(batch.len());
    let mut others = Vec::new();
    {
        let mut counts = HashMap::new();
        for candidate in &batch {
            let count = counts.entry(candidate.bucket.as_str()).or_insert(0);
            *count += 1;
            covers.push(*count <= 4);
        }
    }
    let mut first = Vec::with_capacity(batch.len());
    for (candidate, cover) in batch.into_iter().zip(covers) {
        if cover {
            first.push(candidate);
        } else {
            others.push(candidate);
        }
    }
    first.extend(others);
    first
}

async fn generate(
    state: &CoreRuntime,
    candidate: &Candidate,
    video: bool,
) -> Result<Option<Vec<u8>>, String> {
    let metadata = tokio::fs::metadata(&candidate.path)
        .await
        .map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() != candidate.size as u64 {
        return Ok(None);
    }
    let modified = metadata.modified().map_err(|error| error.to_string())?;
    if indexed_time(modified) != candidate.modified {
        return Ok(None);
    }
    let request = Request {
        path: candidate.path.clone(),
        width: 128,
        height: 128,
        center_crop: video,
        short_edge: None,
        file_id: String::new(),
        file_name: String::new(),
        access_order: None,
    };
    let hash = Arc::new(OnceLock::new());
    let bytes = state
        .thumbnails
        .get_inner(state, request, 0, Some(hash.clone()))
        .await?;
    let Some(bytes) = bytes else { return Ok(None) };
    let hash = match hash.get() {
        Some(generated) => generated.clone(),
        None => plain_libs::media::thumbnails::thumb_hash_from_encoded(bytes)
            .await
            .map_err(|error| error.to_string())?,
    };
    let current = tokio::fs::metadata(&candidate.path)
        .await
        .map_err(|error| error.to_string())?;
    Ok(
        (current.len() == metadata.len() && current.modified().ok() == Some(modified))
            .then_some(hash),
    )
}

fn indexed_time(modified: SystemTime) -> String {
    let millis = modified
        .duration_since(UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default();
    plain_libs::utils::dbtime::iso_from_unix_millis(millis)
}

fn write_batch(db: &Db, batch: &[(Candidate, Vec<u8>)]) -> SqlResult<usize> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut updated = 0;
        {
            let mut statement = tx.prepare_cached(
                "UPDATE files SET thumb_hash=?1
                 WHERE id=?2 AND updated_at=?3 AND size_bytes=?4 AND thumb_hash IS NULL
                 AND kind IN ('IMAGE','VIDEO') AND missing=0 AND indexed=1 AND trashed_at IS NULL
                 AND EXISTS (SELECT 1 FROM file_catalog WHERE file_id=?2 AND path=?5 AND is_available=1)",
            )?;
            for (candidate, hash) in batch {
                updated += statement.execute(params![
                    hash, candidate.id, candidate.modified, candidate.size, candidate.path
                ])?;
            }
        }
        tx.commit()?;
        Ok(updated)
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/images/thumbnails/backfill.rs"]
mod tests;
