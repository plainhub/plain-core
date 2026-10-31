use super::*;

fn fact(id: &str, name: &str, path: &str, size: i64) -> ItemFact {
    ItemFact {
        id: id.into(),
        name: name.into(),
        size,
        path: path.into(),
    }
}

#[test]
fn aggregation_sums_sizes_counts_items_and_keeps_four_newest_paths() {
    let items = (0..6)
        .map(|index| fact("1", "Photos", &format!("/p/{index}"), 10))
        .collect();
    let result = aggregate("IMAGE", items).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].item_count, 6);
    assert_eq!(result[0].size, 60);
    assert_eq!(result[0].top_items, ["/p/0", "/p/1", "/p/2", "/p/3"]);
}

#[test]
fn aggregation_uses_shared_pinyin_sort_keys_and_rejects_invalid_facts() {
    let result = aggregate(
        "DOC",
        vec![fact("2", "照片", "/z/1", 4), fact("1", "Zulu", "/a/1", 3)],
    )
    .unwrap();
    assert_eq!(
        result.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        ["2", "1"]
    );
    assert!(aggregate("CONTACT", vec![]).is_err());
    assert!(aggregate("IMAGE", vec![fact("1", "Photos", "/p", -1)]).is_err());
}

#[test]
fn aggregation_orders_equal_names_deterministically_and_rejects_overflow() {
    let result = aggregate(
        "IMAGE",
        vec![fact("b", "Same", "/b", 0), fact("a", "Same", "/a", 0)],
    )
    .unwrap();
    assert_eq!(
        result
            .iter()
            .map(|bucket| bucket.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert!(
        aggregate(
            "AUDIO",
            vec![
                fact("a", "Audio", "/1", i64::MAX),
                fact("a", "Audio", "/2", 1)
            ]
        )
        .is_err()
    );
    assert!(
        aggregate(
            "VIDEO",
            vec![fact("a", "First", "/1", 1), fact("a", "Other", "/2", 1)]
        )
        .is_err()
    );
}

#[test]
fn sort_keys_are_computed_once_per_bucket_and_shared_across_provider_sources() {
    let mut calls = Vec::new();
    let buckets = aggregate_with_sort_key(
        "IMAGE",
        vec![
            fact("b", "中国", "/b/1", 1),
            fact("a", "Zulu", "/a/1", 1),
            fact("b", "中国", "/b/2", 1),
            fact("a", "Zulu", "/a/2", 1),
        ],
        |name| {
            calls.push(name.to_owned());
            plain_rs::utils::sort_name::sort_name(name)
        },
    )
    .unwrap();
    assert_eq!(calls, ["中国", "Zulu"]);
    assert_eq!(
        buckets
            .iter()
            .map(|bucket| bucket.id.as_str())
            .collect::<Vec<_>>(),
        ["b", "a"]
    );
    assert_eq!(buckets[0].sort_name, "zhongguo");
    assert_eq!(buckets[1].sort_name, "zulu");
    let raw = serde_json::json!([
        {"id":"b", "name":"中国", "path":"content://image/b", "size":2},
        {"id":"a", "name":"Zulu", "path":"content://image/a", "size":2}
    ]);
    let platform = aggregate("IMAGE", serde_json::from_value(raw).unwrap()).unwrap();
    assert_eq!(
        platform
            .iter()
            .map(|bucket| (&bucket.id, &bucket.sort_name))
            .collect::<Vec<_>>(),
        buckets
            .iter()
            .map(|bucket| (&bucket.id, &bucket.sort_name))
            .collect::<Vec<_>>()
    );
}

fn selected_ids(
    plan: plain_rs::services::files::provider_query::Plan,
    fixtures: &str,
) -> Vec<String> {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE source (_id TEXT, mime_type TEXT, _size INTEGER, duration INTEGER, bucket_display_name TEXT);").unwrap();
    connection.execute_batch(fixtures).unwrap();
    let mut statement = connection
        .prepare(&format!(
            "SELECT _id FROM source WHERE {} ORDER BY _id",
            plan.clauses.join(" AND ")
        ))
        .unwrap();
    statement
        .query_map(rusqlite::params_from_iter(plan.args.iter()), |row| {
            row.get(0)
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn document_bucket_plan_reuses_shared_mimes_and_retains_size_and_named_bucket_rules() {
    let plan = platform_plan(plain_rs::enums::DataType::Doc).unwrap();
    assert_eq!(
        plan.args,
        std::iter::once("text/%")
            .chain(plain_rs::media::documents::MIME_TYPES.iter().copied())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    );
    let ids = selected_ids(
        plan,
        "INSERT INTO source VALUES ('text','text/markdown',3,0,'Files'),('pdf','application/pdf',3,0,'Files'),('zip','application/zip',3,0,'Files'),('empty','text/plain',0,0,'Files'),('unnamed','application/pdf',3,0,'');",
    );
    assert_eq!(ids, ["pdf", "text"]);
}

#[test]
fn audio_bucket_plan_keeps_existing_duration_filter_without_applying_it_to_other_media() {
    let fixtures = "INSERT INTO source VALUES ('ready','audio/mpeg',3,100,'Music'),('zero','audio/mpeg',3,0,'Music'),('unnamed','audio/mpeg',3,100,'');";
    assert_eq!(
        selected_ids(
            platform_plan(plain_rs::enums::DataType::Audio).unwrap(),
            fixtures
        ),
        ["ready"]
    );
    for kind in [
        plain_rs::enums::DataType::Image,
        plain_rs::enums::DataType::Video,
    ] {
        assert_eq!(
            selected_ids(platform_plan(kind).unwrap(), fixtures),
            ["ready", "zero"]
        );
    }
    assert!(platform_plan(plain_rs::enums::DataType::Contact).is_err());
}
