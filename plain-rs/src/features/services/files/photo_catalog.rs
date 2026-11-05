use crate::infrastructure::{db::Db, host::bridge::Host};
use serde::Deserialize;
use serde_json::json;

#[derive(Default)]
pub struct PhotoCatalog(tokio::sync::Mutex<Option<(u64, String, usize)>>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Asset {
    id: String,
    name: String,
    media_type: i32,
    duration_ms: i64,
    width: i32,
    height: i32,
    created_at: String,
    updated_at: String,
    favorite: bool,
}

#[derive(Deserialize)]
struct Snapshot {
    revision: String,
    total: usize,
}

impl PhotoCatalog {
    pub async fn sync(&self, db: &Db, host: &Host) -> Result<(), String> {
        let mut current = self.0.lock().await;
        let generation = host.generation().ok_or("host disconnected")?;
        let snapshot: Snapshot =
            serde_json::from_value(host.call("photoCatalogSnapshot", json!({})).await?)
                .map_err(|error| error.to_string())?;
        if current.as_ref() == Some(&(generation, snapshot.revision.clone(), snapshot.total)) {
            return Ok(());
        }
        let mut assets = Vec::new();
        for offset in (0..snapshot.total).step_by(128) {
            let page: Vec<Asset> = serde_json::from_value(
                host.call(
                    "photoCatalogPage",
                    json!({
                        "revision": snapshot.revision, "offset": offset, "limit": 128
                    }),
                )
                .await?,
            )
            .map_err(|error| error.to_string())?;
            if page.len() != 128.min(snapshot.total - offset) {
                return Err("Photo catalog changed during enumeration".into());
            }
            assets.extend(page);
        }
        host.call("photoCatalogVerify", json!({"revision": snapshot.revision}))
            .await?;
        if !host.is_current(generation) {
            return Err("host disconnected".into());
        }
        replace(db, &assets).map_err(|error| error.to_string())?;
        *current = Some((generation, snapshot.revision, snapshot.total));
        Ok(())
    }
}

fn replace(db: &Db, assets: &[Asset]) -> rusqlite::Result<()> {
    use rusqlite::params;
    for asset in assets {
        if asset.id.is_empty()
            || asset.id.contains(['\\', '\0'])
            || asset
                .id
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !matches!(asset.media_type, 1 | 2)
            || asset.duration_ms < 0
            || asset.width < 0
            || asset.height < 0
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('photos','photos','ph://',1) ON CONFLICT(identity) DO UPDATE SET is_available=1", [])?;
        tx.execute("UPDATE files SET missing=1 WHERE volume_id='photos'", [])?;
        for asset in assets {
            let created = plain_libs::utils::dbtime::normalize(&asset.created_at).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let updated = plain_libs::utils::dbtime::normalize(&asset.updated_at).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let id = format!("photos:{}", asset.id);
            let kind = if asset.media_type == 1 { "IMAGE" } else { "VIDEO" };
            let mime = plain_libs::utils::mime::mime_from_ext(&asset.name);
            let changed = tx.execute("UPDATE files SET file_name=?3,created_at=?4,taken_at=?4,updated_at=?5,width=?6,height=?7,title=?3,favorited=?8,kind=?9,duration_ms=?10,mime_type=?11,missing=0,indexed=1 WHERE id=?1 AND volume_id='photos' AND path=?2", params![id, asset.id, asset.name, created, updated, asset.width, asset.height, asset.favorite, kind, asset.duration_ms, mime])?;
            if changed == 0 {
                tx.execute("INSERT INTO files(id,volume_id,path,file_name,kind,size_bytes,created_at,updated_at,width,height,taken_at,title,favorited,duration_ms,mime_type) VALUES(?1,'photos',?2,?3,?9,0,?4,?5,?6,?7,?4,?3,?8,?10,?11)", params![id, asset.id, asset.name, created, updated, asset.width, asset.height, asset.favorite, kind, asset.duration_ms, mime])?;
            }
        }
        tx.commit()
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/photo_catalog.rs"]
mod tests;
