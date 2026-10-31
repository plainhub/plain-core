use crate::kv::Db;
use anyhow::Result;
const KEY: &str = "recent_files";
pub fn add(store: &Db, path: &str) -> Result<()> {
    store.update_json(KEY, |paths: &mut Vec<String>| {
        paths.retain(|value| value != path);
        paths.insert(0, path.to_owned());
        paths.truncate(500);
        Ok(())
    })
}
pub fn list(store: &Db, limit: usize) -> Result<Vec<String>> {
    let mut paths: Vec<String> = store.get_json(KEY)?.unwrap_or_default();
    if limit > 0 {
        paths.truncate(limit);
    }
    Ok(paths)
}
