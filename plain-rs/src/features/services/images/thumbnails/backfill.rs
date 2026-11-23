use super::{CoreRuntime, Job};
use crate::infrastructure::db::{Db, FileKind};
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use plain_libs::rusqlite::{Result as SqlResult, params};
use std::{
    collections::HashMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const BATCH_SIZE: usize = 64;
const READ_BATCH: &str = "WITH pending AS NOT MATERIALIZED (
    SELECT id,path,bucket_id,updated_at,size_bytes,kind FROM file_catalog
    WHERE missing=0 AND trashed_at IS NULL AND indexed=1 AND is_available=1
    AND thumb_hash IS NULL AND kind IN ('IMAGE','VIDEO')
    AND (updated_at,id)<(?1,?2) AND size_bytes>0
)
SELECT * FROM pending WHERE kind='IMAGE'
UNION ALL
SELECT * FROM pending WHERE kind='VIDEO'
ORDER BY updated_at DESC,id DESC LIMIT ?3";

struct Candidate {
    id: String,
    kind: FileKind,
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
    let concurrency = state.thumbnails.workers().saturating_sub(1).clamp(1, 4);
    let mut results = Vec::with_capacity(BATCH_SIZE);
    let mut cursor = None;
    loop {
        let db = state.db.clone();
        let batch = tokio::task::spawn_blocking(move || read_batch(&db, cursor.as_ref())).await??;
        let last_batch = batch.len() < BATCH_SIZE;
        cursor = batch
            .last()
            .map(|last| (last.modified.clone(), last.id.clone()));
        let batch = stream::iter(prioritize_covers(batch))
            .map(|candidate| async move {
                let hash = generate(state, &candidate).await;
                (candidate, hash)
            })
            .buffer_unordered(concurrency);
        tokio::pin!(batch);
        let mut decode_errors = 0;
        while let Some((candidate, hash)) = batch.next().await {
            match hash {
                Ok(Some(hash)) => {
                    results.push((candidate, hash));
                    if results.len() == BATCH_SIZE {
                        flush(state, &mut results).await?;
                    }
                }
                Ok(None) => {}
                Err(_) => decode_errors += 1,
            }
        }
        if decode_errors != 0 {
            log::warn!("Unable to generate {decode_errors} thumbnail hashes in the batch");
        }
        if last_batch {
            break;
        }
    }
    if !results.is_empty() {
        flush(state, &mut results).await?;
    }
    Ok(())
}

async fn flush(
    state: &CoreRuntime,
    results: &mut Vec<(Candidate, Bytes)>,
) -> plain_libs::anyhow::Result<()> {
    let db = state.db.clone();
    let batch = std::mem::take(results);
    let (mut batch, changed) = tokio::task::spawn_blocking(move || {
        let changed = write_batch(&db, &batch)?;
        Ok::<_, plain_libs::rusqlite::Error>((batch, changed))
    })
    .await??;
    batch.clear();
    *results = batch;
    if !changed.is_empty() {
        state.media_scans.publish_catalog_types(changed);
    }
    Ok(())
}

fn read_batch(db: &Db, cursor: Option<&(String, String)>) -> SqlResult<Vec<Candidate>> {
    let (modified, id) = cursor.map_or(("9999", ""), |(modified, id)| {
        (modified.as_str(), id.as_str())
    });
    db.with_conn(|conn| {
        let mut statement = conn.prepare_cached(READ_BATCH)?;
        statement
            .query_map(params![modified, id, BATCH_SIZE as i64], |row| {
                Ok(Candidate {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    bucket: row.get(2)?,
                    modified: row.get(3)?,
                    size: row.get(4)?,
                    kind: row.get(5)?,
                })
            })?
            .collect()
    })
}

fn prioritize_covers(batch: Vec<Candidate>) -> Vec<Candidate> {
    let mut covers = Vec::with_capacity(batch.len());
    let mut others = Vec::new();
    {
        let mut counts = HashMap::new();
        for candidate in &batch {
            let count = counts
                .entry((candidate.bucket.as_str(), candidate.kind.as_str()))
                .or_insert(0);
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

async fn generate(state: &CoreRuntime, candidate: &Candidate) -> Result<Option<Bytes>, String> {
    if !Path::new(&candidate.path).is_absolute() {
        return Err("absolute thumbnail source required".into());
    }
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
    let hash = state
        .thumbnails
        .queue
        .submit(
            Job::ThumbHash {
                path: candidate.path.clone(),
                video: candidate.kind == FileKind::Video,
                size: metadata.len(),
            },
            0,
        )
        .await?;
    let Some(hash) = hash else { return Ok(None) };
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

fn write_batch(db: &Db, batch: &[(Candidate, Bytes)]) -> SqlResult<Vec<i32>> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut changed = Vec::with_capacity(2);
        {
            let mut statement = tx.prepare_cached(
                "UPDATE files SET thumb_hash=?1
                 WHERE id=?2 AND updated_at=?3 AND size_bytes=?4 AND thumb_hash IS NULL
                 AND kind=?6 AND missing=0 AND indexed=1 AND trashed_at IS NULL
                 AND EXISTS (SELECT 1 FROM file_catalog WHERE file_id=?2 AND path=?5 AND is_available=1)",
            )?;
            for (candidate, hash) in batch {
                let updated = statement.execute(params![
                    hash.as_ref(), candidate.id, candidate.modified, candidate.size, candidate.path, candidate.kind
                ])?;
                let kind = candidate.kind.data_type().kind();
                if updated != 0 && !changed.contains(&kind) {
                    changed.push(kind);
                }
            }
        }
        tx.commit()?;
        Ok(changed)
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/images/thumbnails/backfill.rs"]
mod tests;
