use crate::recent_files::{add, list};
const KEY: &str = "recent_files";

#[test]
fn recent_files_deduplicate_move_to_front_and_bound_the_list() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    prefs
        .set_json(
            KEY,
            (0..500)
                .map(|index| format!("/file/{index}"))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    add(&prefs, "/file/200").unwrap();
    let paths = list(&prefs, 0).unwrap();
    assert_eq!(paths.len(), 500);
    assert_eq!(paths[0], "/file/200");
    assert_eq!(paths.iter().filter(|path| *path == "/file/200").count(), 1);
    add(&prefs, "/new").unwrap();
    assert_eq!(list(&prefs, 2).unwrap(), vec!["/new", "/file/200"]);
    assert_eq!(list(&prefs, 0).unwrap().len(), 500);
}

#[test]
fn concurrent_recent_updates_preserve_every_distinct_entry() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    let start = std::sync::Barrier::new(16);
    std::thread::scope(|scope| {
        for index in 0..16 {
            let prefs = &prefs;
            let start = &start;
            scope.spawn(move || {
                start.wait();
                add(prefs, &format!("/file/{index}")).unwrap();
            });
        }
    });
    let paths = list(&prefs, 0).unwrap();
    assert_eq!(paths.len(), 16);
    assert_eq!(
        paths
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        16
    );
}

#[test]
fn invalid_recent_state_is_reported_without_replacing_it() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    prefs.set_json(KEY, "invalid").unwrap();
    assert!(list(&prefs, 10).is_err());
    assert!(add(&prefs, "/file").is_err());
    assert_eq!(prefs.get_json::<String>(KEY).unwrap().unwrap(), "invalid");
}
