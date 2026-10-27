use super::*;

#[test]
fn bookmark_store_survives_reopen_and_ungroups_on_group_delete() {
    let path = crate::db::chat_tests::unique_tmp_dir("bookmarks").join("plain.db");
    let db = Db::open(&path).unwrap();
    let group = DBookmarkGroup::new("Links");
    insert_bookmark_group(&db, &group).unwrap();
    let bookmark = DBookmark::new("https://example.com", &group.id);
    insert_bookmark(&db, &bookmark).unwrap();
    assert_eq!(get_bookmarks_by_group_id(&db, &group.id).len(), 1);
    drop(db);

    let db = Db::open(&path).unwrap();
    assert_eq!(get_bookmarks(&db).len(), 1);
    assert_eq!(get_bookmark_groups(&db).len(), 1);
    delete_bookmark_group(&db, &group.id).unwrap();
    let ungrouped = get_bookmark_by_id(&db, &bookmark.id).unwrap().unwrap();
    assert_eq!(ungrouped.group_id, None);
    assert!(get_bookmark_group_by_id(&db, &group.id).is_none());
    assert_eq!(delete_bookmarks(&db, &[bookmark.id]).unwrap(), 1);
    assert!(get_bookmarks(&db).is_empty());
}

/// Phone Room DAO listing order, locked: pinned first, then sort_order,
/// then creation (2026-09-26 — the server switched to this store and the
/// order must not drift from the phone contract).
#[test]
fn listing_order_pins_first_then_sort_then_created() {
    let db = Db::open(&crate::db::chat_tests::unique_tmp_dir("bm-order").join("plain.db")).unwrap();
    let older = now_iso_minus(3600);
    let mut b1 = DBookmark::new("https://1.example", "");
    b1.sort_order = 2;
    b1.created_at = older.clone();
    b1.updated_at = older.clone();
    let mut b2 = DBookmark::new("https://2.example", "");
    b2.sort_order = 1;
    let mut b3 = DBookmark::new("https://3.example", "");
    b3.sort_order = 1;
    b3.created_at = older.clone();
    b3.updated_at = older.clone();
    b3.pinned = true;
    for b in [&b1, &b2, &b3] {
        insert_bookmark(&db, b).unwrap();
    }
    let mine = [b1.id.clone(), b2.id.clone(), b3.id.clone()];
    let ids: Vec<String> = get_bookmarks(&db)
        .into_iter()
        .map(|b| b.id)
        .filter(|id| mine.contains(id))
        .collect();
    assert_eq!(ids, vec![b3.id.clone(), b2.id.clone(), b1.id.clone()]);
}

fn now_iso_minus(secs: u64) -> String {
    crate::utils::dbtime::iso_from_unix_millis(
        crate::utils::dbtime::now_millis() - secs as i64 * 1000,
    )
}

#[test]
fn explicit_updates_preserve_clicks_and_replace_only_requested_favicons() {
    let db = Db::open(&crate::db::chat_tests::unique_tmp_dir("bookmark-update").join("plain.db"))
        .unwrap();
    let mut original = DBookmark::new("https://example.com/page", "");
    original.favicon_path = "original-icon".into();
    insert_bookmark(&db, &original).unwrap();
    record_click(&db, &original.id).unwrap();

    let (updated, previous_icon) = update_fields(&db, &original, Some("selected-icon"))
        .unwrap()
        .unwrap();
    assert_eq!(previous_icon, "original-icon");
    assert_eq!(updated.favicon_path, "selected-icon");
    assert_eq!(updated.click_count, 1);
    let mut stale_edit = original.clone();
    stale_edit.title = "Explicit title".into();
    let (updated, previous_icon) = update_fields(&db, &stale_edit, None).unwrap().unwrap();
    assert_eq!(updated.title, "Explicit title");
    assert_eq!(updated.favicon_path, "selected-icon");
    assert_eq!(previous_icon, "selected-icon");
    assert_eq!(updated.click_count, 1);

    let (cleared, previous_icon) = update_fields(&db, &updated, Some("")).unwrap().unwrap();
    assert_eq!(previous_icon, "selected-icon");
    assert!(cleared.favicon_path.is_empty());
    delete_bookmarks(&db, std::slice::from_ref(&original.id)).unwrap();
    assert!(
        update_fields(&db, &original, Some("unused-icon"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn bookmark_lookup_distinguishes_missing_rows_from_decode_failures() {
    let db = Db::open(&crate::db::chat_tests::unique_tmp_dir("bookmark-read").join("plain.db")).unwrap();
    assert!(get_bookmark_by_id(&db, "missing").unwrap().is_none());
    let bookmark = DBookmark::new("https://example.com/page", "");
    insert_bookmark(&db, &bookmark).unwrap();
    db.with_conn(|conn| {
        conn.execute("UPDATE bookmarks SET title=?1 WHERE id=?2", params![vec![0u8], bookmark.id])
    }).unwrap();
    assert!(get_bookmark_by_id(&db, &bookmark.id).is_err());
}
