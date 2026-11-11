use super::*;
fn request(root: &Path, query: &str) -> Request {
    Request {
        root: root.to_string_lossy().into_owned(),
        query: query.into(),
        sort_by: SortBy::NameAsc,
        offset: 0,
        limit: None,
        count_only: false,
        count_children: true,
    }
}

#[test]
fn child_directory_reads_follow_the_requested_fields() {
    let root = crate::temporary::tempdir().unwrap();
    let child = root.path().join("Downloads");
    fs::create_dir(&child).unwrap();
    fs::write(child.join("document.txt"), b"fixture").unwrap();
    for selected in [false, true] {
        let mut query = request(root.path(), "");
        query.count_children = selected;
        let mut reads = Vec::new();
        let page = query
            .plan()
            .unwrap()
            .walk_inner(
                None,
                || false,
                Instant::now,
                |path, hidden| {
                    reads.push(path.to_owned());
                    crate::filesystem::count_dir_entries(path, hidden)
                },
            )
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "Downloads");
        assert_eq!(
            reads,
            if selected {
                vec![child.clone()]
            } else {
                Vec::new()
            }
        );
        assert_eq!(page.items[0].child_count, i32::from(selected));
    }
    let mut entered = request(&child, "");
    entered.count_children = false;
    let page = entered.plan().unwrap().execute().unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "document.txt");
}

#[test]
fn literal_name_queries_preserve_text_without_injecting_filters() {
    let root = crate::temporary::tempdir().unwrap();
    for (text, query) in [
        ("white space", r"text:=white\ space"),
        ("field:colon", "text:=field:colon"),
        ("quote\"single'", r#"text:=quote\"single\'"#),
        (r"back\slash", r"text:=back\\slash"),
        (">greater", "text:=>greater"),
        ("=equal", "text:==equal"),
        ("  ", r"text:=\ \ "),
        (
            "needle parent:/ignored show_hidden:true",
            r"text:=needle\ parent:/ignored\ show_hidden:true",
        ),
    ] {
        let plan = request(root.path(), &format!("{query} show_hidden:false"))
            .plan()
            .unwrap();
        assert_eq!(plan.text, text.to_lowercase(), "{query}");
        assert_eq!(Path::new(&plan.root), root.path(), "{query}");
        assert!(!plan.show_hidden, "{query}");
        assert!(plan.recursive, "{query}");
    }
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("nested/white space 中文.txt"), b"owned").unwrap();
    fs::write(root.path().join("nested/unrelated.txt"), b"owned").unwrap();
    let page = request(root.path(), r"text:=white\ space")
        .plan()
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(page.count, 1);
    assert_eq!(page.items[0].name, "white space 中文.txt");
}
#[test]
fn browse_owns_recursive_filters_hidden_counts_order_and_pages() {
    let temp = crate::temporary::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("NeedleDir")).unwrap();
    fs::create_dir(root.join(".hidden")).unwrap();
    fs::write(root.join("NeedleDir/NEEDLE 中文.txt"), b"four").unwrap();
    fs::write(root.join("NeedleDir/.needle-hidden"), b"hidden").unwrap();
    fs::write(root.join(".hidden/needle-secret"), b"secret").unwrap();
    fs::write(root.join("a.txt"), b"a").unwrap();
    let sparse = fs::File::create(root.join("large.bin")).unwrap();
    sparse.set_len(5_000_000_001).unwrap();
    let page = request(root, "").plan().unwrap().execute().unwrap();
    assert_eq!(page.count, 3);
    assert_eq!(page.items[0].name, "NeedleDir");
    assert_eq!(page.items[0].child_count, 1);
    assert_eq!(page.items[2].size, 5_000_000_001);
    let page = request(root, "needle").plan().unwrap().execute().unwrap();
    assert_eq!(page.count, 2);
    assert_eq!(page.items[0].name, "NeedleDir");
    assert_eq!(page.items[1].name, "NEEDLE 中文.txt");
    let page = request(root, "needle show_hidden:true")
        .plan()
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(page.count, 4);
    assert_eq!(page.items[0].child_count, 2);
    let query = format!(
        "parent:{} text:中文 file_size:>=4 file_size:<=4",
        crate::serde_json::json!(root.join("NeedleDir").to_str().unwrap())
    );
    let page = request(&root.join("missing"), &query)
        .plan()
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(page.count, 1);
    assert_eq!(page.items[0].size, 4);
    assert_eq!(
        request(root, "file_size:>5000000000")
            .plan()
            .unwrap()
            .execute()
            .unwrap()
            .items[0]
            .size,
        5_000_000_001
    );
    assert_eq!(
        request(root, "file_size:bad")
            .plan()
            .unwrap()
            .execute()
            .unwrap()
            .count,
        0
    );
    let mut paged = request(root, "");
    paged.offset = 1;
    paged.limit = Some(1);
    assert_eq!(
        paged.plan().unwrap().execute().unwrap().items[0].name,
        "a.txt"
    );
    let mut zero = request(root, "");
    zero.limit = Some(0);
    let page = zero.plan().unwrap().execute().unwrap();
    assert_eq!(page.count, 3);
    assert!(page.items.is_empty());
    let mut count = request(root, "needle");
    count.count_only = true;
    let page = count.plan().unwrap().execute().unwrap();
    assert_eq!(page.count, 2);
    assert!(page.items.is_empty());
    assert_eq!(
        request(&root.join("missing"), "")
            .plan()
            .unwrap()
            .execute()
            .unwrap()
            .count,
        0
    );
}
#[test]
fn browse_does_not_follow_recursive_symlink_cycles_or_escape_the_root() {
    let temp = crate::temporary::tempdir().unwrap();
    let outside = crate::temporary::tempdir().unwrap();
    fs::write(outside.path().join("needle-secret"), b"outside").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(temp.path(), temp.path().join("cycle")).unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("outside")).unwrap();
        assert_eq!(
            request(temp.path(), "needle")
                .plan()
                .unwrap()
                .execute()
                .unwrap()
                .count,
            0
        );
        assert_eq!(
            request(temp.path(), "")
                .plan()
                .unwrap()
                .execute()
                .unwrap()
                .count,
            2
        );
    }
}

#[test]
fn progressive_search_keeps_exact_counts_and_sorted_pages_and_cancels_before_more_reads() {
    let root = crate::temporary::tempdir().unwrap();
    fs::create_dir(root.path().join("node_modules")).unwrap();
    for index in 0..300 {
        fs::write(
            root.path()
                .join("node_modules")
                .join(format!("needle-{index:03}.rs")),
            b"source",
        )
        .unwrap();
    }
    let mut request = request(root.path(), "needle");
    request.offset = 10;
    request.limit = Some(5);
    let plan = request.plan().unwrap();
    let mut snapshots = Vec::new();
    let page = plan
        .walk(
            |page| {
                snapshots.push(page);
                true
            },
            || false,
        )
        .unwrap();
    assert_eq!(page.count, 300);
    assert_eq!(
        page.items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        [
            "needle-010.rs",
            "needle-011.rs",
            "needle-012.rs",
            "needle-013.rs",
            "needle-014.rs"
        ]
    );
    assert!(!snapshots.is_empty());
    assert!(
        snapshots
            .iter()
            .all(|page| page.count < 300 && page.items.len() == 5)
    );
    let mut reads = 0;
    let error = plan
        .walk(
            |page| {
                reads += 1;
                assert!(page.count < 300);
                false
            },
            || false,
        )
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert_eq!(reads, 1);
    assert_eq!(
        plan.walk(|_| panic!("cancelled before traversal"), || true)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[cfg(unix)]
#[test]
fn recursive_search_skips_denied_children_but_rejects_a_denied_root() {
    use std::os::unix::fs::PermissionsExt;
    let root = crate::temporary::tempdir().unwrap();
    let denied = root.path().join("denied");
    fs::create_dir(&denied).unwrap();
    fs::write(denied.join("needle-private"), b"private").unwrap();
    fs::write(root.path().join("needle-readable"), b"readable").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0)).unwrap();
    let search = request(root.path(), "needle").plan().unwrap().execute();
    let direct = request(&denied, "needle").plan().unwrap().execute();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        direct.err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    let page = search.unwrap();
    assert_eq!(page.count, 1);
    assert_eq!(page.items[0].name, "needle-readable");
}

#[test]
fn progressive_search_limits_snapshots_and_avoids_duplicate_empty_pages() {
    let root = crate::temporary::tempdir().unwrap();
    for index in 0..1024 {
        fs::write(root.path().join(format!("needle-{index:04}")), b"fixture").unwrap();
    }
    let plan = request(root.path(), "needle").plan().unwrap();
    let mut snapshots = Vec::new();
    let now = Instant::now();
    let page = plan
        .walk_inner(
            Some(&mut |page| {
                snapshots.push(page);
                true
            }),
            || false,
            || now,
            crate::filesystem::count_dir_entries,
        )
        .unwrap();
    assert_eq!(page.count, 1024);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].count, 128);
    let mut ticks = 0;
    let mut snapshots = Vec::new();
    let page = plan
        .walk_inner(
            Some(&mut |page| {
                snapshots.push(page);
                true
            }),
            || false,
            || {
                ticks += 1;
                now + Duration::from_millis(ticks * 100)
            },
            crate::filesystem::count_dir_entries,
        )
        .unwrap();
    assert_eq!(page.count, 1024);
    assert_eq!(snapshots.len(), 7);
}

#[test]
fn no_match_search_emits_no_repeated_empty_snapshots() {
    let root = crate::temporary::tempdir().unwrap();
    for index in 0..512 {
        fs::write(root.path().join(format!("file-{index:04}")), b"fixture").unwrap();
    }
    let page = request(root.path(), "missing")
        .plan()
        .unwrap()
        .walk(|_| panic!("no new matches to send"), || false)
        .unwrap();
    assert_eq!(page.count, 0);
}

#[test]
#[ignore]
fn large_directory_search_measurements() {
    let root = crate::temporary::tempdir().unwrap();
    for branch in 0..100 {
        let directory = root.path().join(format!("branch-{branch:03}"));
        fs::create_dir(&directory).unwrap();
        for index in 0..200 {
            fs::write(
                directory.join(format!("needle-{branch:03}-{index:03}.txt")),
                b"fixture",
            )
            .unwrap();
        }
    }
    for query in ["needle", "missing", "file_size:>=1"] {
        let mut request = request(root.path(), query);
        request.limit = Some(10000);
        let start = Instant::now();
        let mut snapshots = 0;
        let mut bytes = 0;
        let page = request
            .plan()
            .unwrap()
            .walk(
                |page| {
                    snapshots += 1;
                    bytes += crate::serde_json::to_vec(&page).unwrap().len();
                    true
                },
                || false,
            )
            .unwrap();
        bytes += crate::serde_json::to_vec(&page).unwrap().len();
        eprintln!(
            "query={query} count={} snapshots={snapshots} bytes={bytes} elapsed_ms={}",
            page.count,
            start.elapsed().as_millis()
        );
        assert_eq!(page.count, if query == "missing" { 0 } else { 20000 });
    }
}
