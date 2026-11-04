use rusqlite::{Connection, Result};
use std::{
    collections::HashMap,
    sync::{
        RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

type Locations = HashMap<String, (String, String)>;
type Volumes = HashMap<String, String>;

struct Snapshot {
    files: Locations,
    volumes: Volumes,
    error: Option<String>,
}

pub(super) struct MediaPaths {
    snapshot: RwLock<Snapshot>,
    files_changed: AtomicBool,
    volumes_changed: AtomicBool,
}

const VISIBLE: &str =
    "kind IN ('IMAGE','VIDEO','AUDIO','DOC') AND missing=0 AND trashed_at IS NULL";

// Project visibility after the primary-key seek so SQLite cannot select the broad kind index.
static CHANGED_LOCATIONS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "SELECT d.id,CASE WHEN {VISIBLE} THEN f.volume_id END,f.path FROM temp.media_path_changes d LEFT JOIN files f ON f.id=d.id"
    )
});

impl MediaPaths {
    pub(super) fn new(conn: &Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TEMP TABLE media_path_changes(id TEXT PRIMARY KEY NOT NULL);
            CREATE TEMP TRIGGER media_path_delete AFTER DELETE ON main.files
            WHEN OLD.kind IN ('IMAGE','VIDEO','AUDIO','DOC')
            BEGIN INSERT OR IGNORE INTO media_path_changes VALUES(OLD.id); END;",
        )?;
        for (event, changed, old) in [
            ("INSERT", "", ""),
            (
                "UPDATE",
                "WHEN (OLD.id,OLD.rowid,OLD.volume_id,OLD.path,OLD.kind,OLD.missing,OLD.trashed_at) IS NOT (NEW.id,NEW.rowid,NEW.volume_id,NEW.path,NEW.kind,NEW.missing,NEW.trashed_at)",
                "INSERT OR IGNORE INTO media_path_changes SELECT OLD.id WHERE OLD.kind IN ('IMAGE','VIDEO','AUDIO','DOC');",
            ),
        ] {
            // REPLACE may remove a conflicting row without firing its delete hook.
            conn.execute_batch(&format!("CREATE TEMP TRIGGER media_path_{event} BEFORE {event} ON main.files {changed} BEGIN
                {old}
                INSERT OR IGNORE INTO media_path_changes SELECT NEW.id WHERE NEW.kind IN ('IMAGE','VIDEO','AUDIO','DOC');
                INSERT OR IGNORE INTO media_path_changes SELECT id FROM files WHERE (rowid=NEW.rowid OR id=NEW.id OR (NEW.missing=0 AND volume_id=NEW.volume_id AND path=NEW.path AND missing=0)) AND {VISIBLE};
                END;"))?;
        }
        let files = conn
            .prepare(&format!(
                "SELECT id,volume_id,path FROM files WHERE {VISIBLE}"
            ))?
            .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
            .collect::<Result<_>>()?;
        Ok(Self {
            snapshot: RwLock::new(Snapshot {
                files,
                volumes: Self::volumes(conn)?,
                error: None,
            }),
            files_changed: false.into(),
            volumes_changed: false.into(),
        })
    }

    fn volumes(conn: &Connection) -> Result<Volumes> {
        conn.prepare("SELECT id,path FROM volumes WHERE is_available=1")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
    }

    pub(super) fn changed(&self, database: &str, table: &str) {
        match (database, table) {
            ("temp", "media_path_changes") => self.files_changed.store(true, Ordering::Release),
            ("main", "volumes") => self.volumes_changed.store(true, Ordering::Release),
            _ => {}
        }
    }

    pub(super) fn publish(&self, conn: &Connection) {
        let files = self.files_changed.load(Ordering::Acquire);
        let volumes = self.volumes_changed.load(Ordering::Acquire);
        if !(files || volumes) || !conn.is_autocommit() {
            return;
        }
        let result = (|| -> Result<()> {
            let roots = volumes.then(|| Self::volumes(conn)).transpose()?;
            let mut delta = Vec::new();
            if files {
                let mut stmt = conn.prepare_cached(&CHANGED_LOCATIONS)?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let volume: Option<String> = row.get(1)?;
                    delta.push((
                        row.get::<_, String>(0)?,
                        volume.map(|v| row.get(2).map(|p| (v, p))).transpose()?,
                    ));
                }
                drop(rows);
                conn.execute("DELETE FROM temp.media_path_changes", [])?;
            }
            let mut view = self.snapshot.write().unwrap();
            if let Some(roots) = roots {
                view.volumes = roots;
            }
            for (id, location) in delta {
                match location {
                    Some(location) => {
                        view.files.insert(id, location);
                    }
                    None => {
                        view.files.remove(&id);
                    }
                }
            }
            view.error = None;
            self.files_changed.store(false, Ordering::Release);
            self.volumes_changed.store(false, Ordering::Release);
            Ok(())
        })();
        if let Err(error) = result {
            log::error!("Cannot publish media paths: {error}");
            self.snapshot.write().unwrap().error = Some(error.to_string());
        }
    }

    fn resolve(&self, id: &str) -> anyhow::Result<String> {
        let view = self.snapshot.read().unwrap();
        if let Some(error) = &view.error {
            anyhow::bail!("{error}");
        }
        let (volume, path) = view
            .files
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Media unavailable"))?;
        let root = view
            .volumes
            .get(volume)
            .ok_or_else(|| anyhow::anyhow!("Media unavailable"))?;
        Ok(if root.ends_with('/') {
            format!("{root}{path}")
        } else {
            format!("{root}/{path}")
        })
    }
}

impl super::Db {
    pub(crate) fn media_resource_path(&self, id: &str) -> anyhow::Result<String> {
        self.1.media_paths.resolve(id)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/db/media_paths.rs"]
mod tests;
