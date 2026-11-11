use crate::infrastructure::db::DNearbyDeviceCache;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::now_iso;
use crate::infrastructure::db::repositories::chat_store::validate;
use plain_libs::anyhow::Result;
use plain_libs::rusqlite::params;
pub fn all(db: &Db) -> Result<Vec<DNearbyDeviceCache>> {
    Ok(db.with_conn(|c|{
 let mut s=c.prepare("SELECT id,name,ips,port,device_type,version,platform,last_seen_at FROM nearby_devices ORDER BY julianday(last_seen_at) DESC,id")?;
 s.query_map([],|r|Ok(DNearbyDeviceCache {id:r.get(0)?,name:r.get(1)?,ips:plain_libs::serde_json::from_str(&r.get::<_,String>(2)?).map_err(|error| plain_libs::rusqlite::Error::FromSqlConversionFailure(2,plain_libs::rusqlite::types::Type::Text,Box::new(error)))?,port:r.get(3)?,device_type:r.get(4)?,version:r.get(5)?,platform:r.get(6)?,last_seen_at:r.get(7)?}))?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()
 })?)
}
pub fn save(db: &Db, row: &DNearbyDeviceCache) -> Result<()> {
    validate(&row.id, &row.last_seen_at, &row.last_seen_at)?;
    let ips = plain_libs::serde_json::to_string(&row.ips.iter().collect::<std::collections::BTreeSet<_>>())?;
    db.with_conn(|c|c.execute("INSERT INTO nearby_devices(id,name,ips,port,device_type,version,platform,last_seen_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET name=excluded.name,ips=excluded.ips,port=excluded.port,device_type=excluded.device_type,version=excluded.version,platform=excluded.platform,last_seen_at=excluded.last_seen_at WHERE julianday(excluded.last_seen_at)>=julianday(nearby_devices.last_seen_at)",params![row.id,row.name,ips,row.port,row.device_type,row.version,row.platform,plain_libs::utils::dbtime::normalize(&row.last_seen_at)?]))?;
    Ok(())
}
pub fn touch(db: &Db, id: &str) -> Result<bool> {
    Ok(db.with_conn(|c| {
        c.execute(
            "UPDATE nearby_devices SET last_seen_at=?2 WHERE id=?1",
            params![id, now_iso()],
        )
    })? == 1)
}
pub fn delete(db: &Db, id: &str) -> Result<bool> {
    Ok(db.with_conn(|c| c.execute("DELETE FROM nearby_devices WHERE id=?1", [id]))? == 1)
}
