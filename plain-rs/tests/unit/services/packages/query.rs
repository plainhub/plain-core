use super::*;
use serde_json::json;

fn package(id: &str, name: &str, kind: &str, size: i64, time: &str) -> Value {
    json!({"id":id,"name":name,"type":kind,"size":size,"updatedAt":time,"certs":[{"issuer":"Certificate % issuer","subject":"subject"}]})
}
#[test]
fn packages_filter_certificates_types_and_page_after_sorting() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db.sqlite")).unwrap();
    let facts = vec![
        package("a", "Zeta", "USER", 5, "2026-10-05T00:00:00.000Z"),
        package("b", "Alpha", "SYSTEM", 9, "2026-10-05T02:00:00+02:00"),
        package("c", "Beta", "USER", 7, "2026-10-05T01:00:00.000Z"),
    ];
    let result = packages(&db, facts.clone(), "type:USER text:'% issuer'", "SIZE_DESC").unwrap();
    assert_eq!(
        result.into_iter().skip(1).take(1).collect::<Vec<_>>()[0]["id"],
        "a"
    );
    assert_eq!(
        packages(&db, facts.clone(), "ids:a,c", "DATE_DESC").unwrap()[0]["id"],
        "c"
    );
    assert!(
        packages(&db, facts.clone(), "text:'% nonmatch'", "NAME_ASC")
            .unwrap()
            .is_empty()
    );
    assert_eq!(packages(&db, facts, "", "NAME_DESC").unwrap()[0]["id"], "a");
}

#[test]
fn names_use_shared_pinyin_in_both_directions_and_keep_equal_names_stable() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db.sqlite")).unwrap();
    let facts = vec![
        package("z", "照片", "USER", 0, ""),
        package("y", "音乐", "USER", 0, ""),
        package("a", "Alpha", "USER", 0, ""),
        package("a2", "ALPHA", "USER", 0, ""),
    ];
    for (order, expected) in [
        ("NAME_ASC", vec!["a", "a2", "y", "z"]),
        ("NAME_DESC", vec!["z", "y", "a", "a2"]),
    ] {
        let result = packages(&db, facts.clone(), "", order).unwrap();
        assert_eq!(
            result
                .iter()
                .map(|item| item["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn sorting_computes_each_name_key_once_and_skips_keys_for_other_orders() {
    let facts = vec![
        package("z", "Zeta", "USER", 3, "2026-10-05T00:00:00Z"),
        package("a", "Alpha", "USER", 1, "2026-10-04T00:00:00Z"),
        package("b", "Beta", "USER", 2, "2026-10-03T00:00:00Z"),
    ];
    for order in [
        "NAME_ASC",
        "NAME_DESC",
        "SIZE_ASC",
        "SIZE_DESC",
        "DATE_ASC",
        "DATE_DESC",
    ] {
        let mut calls = Vec::new();
        let result = sort_packages(facts.clone(), order, |name| {
            calls.push(name.to_owned());
            name.to_lowercase()
        });
        assert_eq!(
            calls.len(),
            if order.starts_with("NAME") {
                facts.len()
            } else {
                0
            },
            "{order}"
        );
        assert_eq!(
            result[0]["id"],
            match order {
                "NAME_ASC" | "SIZE_ASC" => "a",
                "DATE_ASC" => "b",
                _ => "z",
            }
        );
    }
}
