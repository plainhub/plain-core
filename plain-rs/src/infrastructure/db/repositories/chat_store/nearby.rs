use crate::features::discovery::nearby_devices::MAX_AGE;
use crate::infrastructure::db::DNearbyDeviceCache;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::repositories::chat_store::validate;
use plain_libs::anyhow::Result;
use plain_libs::chrono::{DateTime, Utc};
use plain_libs::rusqlite::{Error, params, types::Type};

pub fn all(db: &Db) -> Result<Vec<DNearbyDeviceCache>> {
    read(db, None)
}

pub fn recent(db: &Db, now: DateTime<Utc>) -> Result<Vec<DNearbyDeviceCache>> {
    read(db, Some((now - MAX_AGE).to_rfc3339()))
}

fn read(db: &Db, cutoff: Option<String>) -> Result<Vec<DNearbyDeviceCache>> {
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(
            "SELECT id,name,ips,port,device_type,version,platform,last_seen_at,remote_access,signature_public_key
             FROM nearby_devices WHERE ?1 IS NULL OR julianday(last_seen_at) >= julianday(?1)
             ORDER BY julianday(last_seen_at) DESC,id",
        )?;
        statement
            .query_map([cutoff], |row| {
                Ok(DNearbyDeviceCache {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    ips: plain_libs::serde_json::from_str(&row.get::<_, String>(2)?).map_err(|error| {
                        Error::FromSqlConversionFailure(2, Type::Text, Box::new(error))
                    })?,
                    port: row.get(3)?,
                    device_type: row.get(4)?,
                    version: row.get(5)?,
                    platform: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    remote_access: row.get(8)?,
                    signature_public_key: row.get(9)?,
                })
            })?
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?)
}

pub fn save(db: &Db, row: &DNearbyDeviceCache) -> Result<()> {
    validate(&row.id, &row.last_seen_at, &row.last_seen_at)?;
    let ips = plain_libs::serde_json::to_string(
        &row.ips.iter().collect::<std::collections::BTreeSet<_>>(),
    )?;
    let last_seen_at = plain_libs::utils::dbtime::normalize(&row.last_seen_at)?;
    db.with_conn(|conn| conn.execute(
        "INSERT INTO nearby_devices(id,name,ips,port,device_type,version,platform,last_seen_at,remote_access,signature_public_key)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name,ips=excluded.ips,port=excluded.port,
           device_type=excluded.device_type,version=excluded.version,platform=excluded.platform,
           last_seen_at=excluded.last_seen_at,remote_access=excluded.remote_access,
           signature_public_key=excluded.signature_public_key
         WHERE julianday(excluded.last_seen_at)>=julianday(nearby_devices.last_seen_at)",
        params![row.id,row.name,ips,row.port,row.device_type,row.version,row.platform,last_seen_at,row.remote_access,row.signature_public_key],
    ))?;
    Ok(())
}
pub fn delete(db: &Db, id: &str) -> Result<bool> {
    Ok(db.with_conn(|c| c.execute("DELETE FROM nearby_devices WHERE id=?1", [id]))? == 1)
}
