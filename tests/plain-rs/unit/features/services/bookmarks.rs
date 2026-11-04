use super::*;

#[test]
fn manual_add_validates_the_whole_batch_before_writing_and_keeps_valid_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    for invalid in ["http://", "javascript:alert(1)", "file:///owned.txt"] {
        assert!(
            add_bookmarks(
                &db,
                vec!["https://example.com/first".into(), invalid.into()],
                ""
            )
            .is_err()
        );
        assert!(bookmark::get_bookmarks(&db).is_empty());
    }
    let added = add_bookmarks(
        &db,
        vec![
            " https://example.com/ ".into(),
            " ".into(),
            "https://example.com/".into(),
        ],
        "",
    )
    .unwrap();
    assert_eq!(added.len(), 2);
    assert_ne!(added[0].id, added[1].id);
    assert!(added.iter().all(|row| row.url == "https://example.com/"));
}

#[test]
fn import_keeps_titles_groups_and_existing_rows_and_deduplicates_repeated_files() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let group = bookmark::DBookmarkGroup::new("Saved & 中文");
    bookmark::insert_bookmark_group(&db, &group).unwrap();
    let mut existing = bookmark::DBookmark::new("https://example.com/existing", &group.id);
    existing.title = "Keep chosen title".into();
    existing.pinned = true;
    existing.click_count = 7;
    bookmark::insert_bookmark(&db, &existing).unwrap();
    let before = serde_json::to_value(&existing).unwrap();
    let html = "<!DOCTYPE NETSCAPE-Bookmark-file-1><DL><p>
      <DT><A HREF='https://example.com/root'>Root &amp; 中文</A>
      <DT><H3>Saved &amp; 中文</H3><DL><p>
        <DT><A HREF='https://example.com/existing'>Do not overwrite</A>
        <DT><A HREF='https://example.com/new?x=1&amp;y=2'>Original &lt;title&gt; 中文</A>
        <DT><A HREF='https://example.com/new?x=1&amp;y=2'>Duplicate title</A>
      </DL><p>
      <DT><H3>New group</H3><DL><p>
        <DT><A HREF='https://example.com/another'></A>
        <DT><A HREF='javascript:alert(1)'>Rejected</A>
        <DT><A HREF='http://'>Malformed</A>
      </DL><p></DL>";
    assert_eq!(import_bookmarks(&db, html).unwrap(), 3);
    let rows = bookmark::get_bookmarks(&db);
    assert_eq!(rows.len(), 4);
    assert_eq!(
        serde_json::to_value(
            bookmark::get_bookmark_by_id(&db, &existing.id)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        before
    );
    let root = rows.iter().find(|row| row.url.ends_with("/root")).unwrap();
    assert_eq!(root.title, "Root & 中文");
    assert_eq!(root.group_id, None);
    let new = rows
        .iter()
        .find(|row| row.url.ends_with("new?x=1&y=2"))
        .unwrap();
    assert_eq!(new.title, "Original <title> 中文");
    assert_eq!(new.group_id.as_deref(), Some(group.id.as_str()));
    let other = rows
        .iter()
        .find(|row| row.url.ends_with("/another"))
        .unwrap();
    assert_eq!(other.title, other.url);
    let groups = bookmark::get_bookmark_groups(&db);
    assert_eq!(groups.len(), 2);
    assert_eq!(
        other.group_id.as_deref(),
        Some(
            groups
                .iter()
                .find(|row| row.name == "New group")
                .unwrap()
                .id
                .as_str()
        )
    );
    let once = serde_json::to_value((&rows, &groups)).unwrap();
    assert_eq!(import_bookmarks(&db, html).unwrap(), 0);
    assert_eq!(
        serde_json::to_value((
            bookmark::get_bookmarks(&db),
            bookmark::get_bookmark_groups(&db)
        ))
        .unwrap(),
        once
    );
}

#[test]
fn import_rolls_back_bookmarks_and_new_groups_on_storage_failure() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    db.with_conn(|conn| conn.execute_batch("CREATE TRIGGER reject_import BEFORE INSERT ON bookmarks WHEN NEW.url='https://example.com/fail' BEGIN SELECT RAISE(ABORT,'expected import failure'); END;")).unwrap();
    assert!(import_bookmarks(&db, "<DL><DT><H3>New group</H3><DL><DT><A HREF='https://example.com/first'>First</A><DT><A HREF='https://example.com/fail'>Fail</A></DL></DL>").is_err());
    assert!(bookmark::get_bookmarks(&db).is_empty());
    assert!(bookmark::get_bookmark_groups(&db).is_empty());
}

#[test]
fn page_metadata_returns_page_values_and_resolves_icons_without_caching() {
    let metadata = parse_page_metadata(
        "<title>Fallback</title><meta property='og:title' content=' Chosen &amp; title '><link rel='shortcut icon' href='../icon.svg'>",
        "https://example.com/pages/article",
    );
    assert_eq!(metadata.title.as_deref(), Some("Chosen & title"));
    assert_eq!(
        metadata.favicon_url.as_deref(),
        Some("https://example.com/icon.svg")
    );
}

#[test]
fn missing_page_values_remain_optional_and_title_limits_count_characters() {
    let empty = parse_page_metadata("<html></html>", "https://example.com/page");
    assert_eq!(empty.title, None);
    assert_eq!(
        empty.favicon_url.as_deref(),
        Some("https://example.com/favicon.ico")
    );
    let metadata = parse_page_metadata(
        &format!("<title> {} </title>", "界".repeat(250)),
        "https://example.com/page",
    );
    assert_eq!(metadata.title.unwrap().chars().count(), 200);
}

#[tokio::test]
async fn unsupported_page_urls_are_rejected_before_fetching() {
    for url in ["file:///page.html", "ftp://example.com/page", "invalid"] {
        assert!(fetch_page_metadata(url).await.is_err(), "{url}");
    }
}
