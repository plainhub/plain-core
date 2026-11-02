use super::*;
use std::path::Path;

#[test]
fn changed_locations_seek_each_file_by_its_unique_id() {
    let db = super::super::Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|conn| {
        let plan = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {}", &*CHANGED_LOCATIONS))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert!(
            plan.iter().any(|line| line.contains("SEARCH f ") && line.contains("(id=?)")),
            "{plan:?}"
        );
    });
}
