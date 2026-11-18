use plain_libs::rusqlite::params;

use crate::infrastructure::db::Db;

impl Db {
    pub fn delete_cached_nearby_device_if_last_seen_matches(
        &self,
        id: &str,
        last_seen_at: &str,
    ) -> plain_libs::rusqlite::Result<bool> {
        let conn = self.0.lock().unwrap();
        Ok(conn.execute(
            "DELETE FROM nearby_devices WHERE id=?1 AND last_seen_at=?2",
            params![id, last_seen_at],
        )? == 1)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/nearby_device.rs"]
mod tests;
