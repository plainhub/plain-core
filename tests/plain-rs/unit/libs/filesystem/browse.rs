use super::*;
fn request(root: &Path, query: &str) -> Request {
    Request {
        root: root.to_string_lossy().into_owned(),
        query: query.into(),
        sort_by: SortBy::NameAsc,
        offset: 0,
        limit: None,
        count_only: false,
    }
}

#[test]
fn literal_name_queries_preserve_text_without_injecting_filters() {
    let root = tempfile::tempdir().unwrap();
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
    let temp = tempfile::tempdir().unwrap();
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
        serde_json::json!(root.join("NeedleDir").to_str().unwrap())
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
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
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
