use rusqlite::params;

use crate::db::DNearbyDeviceCache;
use crate::db::Db;

impl Db {
    pub fn get_cached_nearby_devices(&self) -> rusqlite::Result<Vec<DNearbyDeviceCache>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,name,ips,port,device_type,version,platform,last_seen_at \
             FROM nearby_devices ORDER BY last_seen_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let ips: String = row.get(2)?;
            Ok(DNearbyDeviceCache {
                id: row.get(0)?,
                name: row.get(1)?,
                ips: serde_json::from_str(&ips).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                port: row.get::<_, i64>(3)? as u16,
                device_type: row.get(4)?,
                version: row.get(5)?,
                platform: row.get(6)?,
                last_seen_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn save_cached_nearby_device(&self, device: &DNearbyDeviceCache) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO nearby_devices (id,name,ips,port,device_type,version,platform,last_seen_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, ips=excluded.ips, port=excluded.port,
               device_type=excluded.device_type, version=excluded.version,
               platform=excluded.platform,
               last_seen_at=excluded.last_seen_at",
            params![device.id, device.name, serde_json::to_string(&device.ips.iter().collect::<std::collections::BTreeSet<_>>()).map_err(|error|rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?, device.port as i64,
                device.device_type, device.version, device.platform, crate::utils::dbtime::normalize(&device.last_seen_at)?],
        )?;
        Ok(())
    }

    pub fn refresh_cached_nearby_device_if_last_seen_matches(
        &self,
        id: &str,
        last_seen_at: &str,
        now: &str,
    ) -> rusqlite::Result<bool> {
        let conn = self.0.lock().unwrap();
        Ok(conn.execute(
            "UPDATE nearby_devices SET last_seen_at=?3 WHERE id=?1 AND last_seen_at=?2",
            params![id, last_seen_at, now],
        )? == 1)
    }

    pub fn delete_cached_nearby_device_if_last_seen_matches(
        &self,
        id: &str,
        last_seen_at: &str,
    ) -> rusqlite::Result<bool> {
        let conn = self.0.lock().unwrap();
        Ok(conn.execute(
            "DELETE FROM nearby_devices WHERE id=?1 AND last_seen_at=?2",
            params![id, last_seen_at],
        )? == 1)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/nearby_device.rs"]
mod tests;
