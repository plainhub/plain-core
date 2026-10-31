use super::*;
use plain_rs::host::bridge::Host;
use std::sync::Arc;
#[test]
fn archived_conversation_search_matches_address_and_snippet_case_insensitively() {
    let addresses = vec!["+1 555 0100".to_owned()];
    assert!(archived_matches(
        &addresses,
        "Old project update",
        "555 0100"
    ));
    assert!(archived_matches(
        &addresses,
        "Old project update",
        "PROJECT"
    ));
    assert!(archived_matches(&addresses, "Old project update", "  "));
    assert!(!archived_matches(
        &addresses,
        "Old project update",
        "missing"
    ));
}

fn db(dir: &tempfile::TempDir) -> Db {
    Db::open(&dir.path().join("db")).unwrap()
}
#[test]
fn sms_mms_plans_partition_ids_literal_text_trash_and_archive_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(&dir);
    db.archived_conversation_save(&plain_rs::db::ArchivedConversationRow {
        conversation_id: "4".into(),
        archived_at: "2026-10-05T01:00:00.123Z".into(),
    })
    .unwrap();
    db.trashed_message_save_many(&[
        plain_rs::db::TrashedMessageRow {
            message_id: "2".into(),
            is_mms: true,
            trashed_at: "2026-01-01T00:00:00.000Z".into(),
        },
        plain_rs::db::TrashedMessageRow {
            message_id: "1".into(),
            is_mms: false,
            trashed_at: "2026-01-01T00:00:00.000Z".into(),
        },
    ])
    .unwrap();
    let fields =
        provider_plan::fields(&db, "thread_id:4 archived:1 ids:1,mms_2 text:'%_'").unwrap();
    let result = plans(&db, &fields, false, Some(vec!["2".into()])).unwrap();
    assert!(result.sms.clauses.contains(&"date <= ?".into()));
    assert!(result.sms.args.contains(&"\\%\\_".into()));
    assert_eq!(result.sms.args.last().unwrap(), "1791162000123");
    let mms = result.mms.unwrap();
    assert_eq!(mms.args.last().unwrap(), "1791162000");
    assert!(!mms.clauses.join(" ").contains("mms_"));
    assert!(mms.clauses.contains(&"m_type IN (128,132)".into()));
    let fields = provider_plan::fields(&db, "ids:mms_bad").unwrap();
    let result = plans(&db, &fields, false, None).unwrap();
    assert!(result.mms.is_none());
    assert!(result.sms.args.contains(&"-1".into()));
}
#[test]
fn merged_messages_sort_real_instants_then_page_and_resolve_blank_addresses() {
    let items = vec![
        json!({"id":"sms","date":"2026-10-05T01:00:00.000Z","address":""}),
        json!({"id":"mms","date":"2026-10-05T04:00:00+02:00","address":"123"}),
    ];
    let result = page(items, 1, 1, "canonical").unwrap();
    assert_eq!(result[0]["id"], "sms");
    assert_eq!(result[0]["address"], "canonical");
    assert_eq!(
        text_ids(
            json!({"1":["Hello","World"],"2":["hello wrong"]}),
            &["hello".into(), "world".into()]
        )
        .unwrap(),
        vec!["1"]
    );
    assert!(page(vec![json!({"date":"bad"})], 0, 1, "").is_err());
}

/// The path that turns a query into the id set `trashSms` / `restoreSms` /
/// `deleteSms` act on used to drop every unrecognised field, which left the
/// clause list empty, and `ContentWhere` renders an empty clause list as
/// `1=1` — so a misspelt filter covered the whole table.
#[test]
fn a_destructive_query_that_builds_no_clause_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();

    // Every spelling of "names nothing that narrows": a misspelt field, the
    // names that merely travel along from a media view, and `archived` — which
    // only contributes a clause when `thread_id` also matches a stored archive
    // record, so on its own it builds nothing. A list of field names would call
    // the last one a selector and let the whole table through.
    for query in [
        "zzz_no_such_field:x",
        "trash:false",
        "bucket_id:7",
        "archived:1",
    ] {
        let fields = provider_plan::fields(&db, query).unwrap();
        let plans = plans(&db, &fields, true, None).unwrap();
        assert!(
            plans.sms.clauses.is_empty(),
            "{query:?} was expected to build no clause at all"
        );
        assert!(
            narrows_nothing(query, &fields, &plans.sms),
            "{query:?} would reach deleteSms with nothing selected, and the whole table is in \
             range"
        );
    }

    // The fields that do build a clause, so a real selection goes through.
    for query in [
        "ids:1,2",
        "text:hello",
        "type:1",
        "thread_id:7",
        "trashed:1",
    ] {
        let fields = provider_plan::fields(&db, query).unwrap();
        let plans = plans(&db, &fields, true, None).unwrap();
        assert!(
            !plans.sms.clauses.is_empty(),
            "{query:?} was expected to build a clause"
        );
        assert!(!narrows_nothing(query, &fields, &plans.sms));
    }
    // `archived` next to the `thread_id` it needs is narrowed by that thread id.
    let paired = provider_plan::fields(&db, "archived:1 thread_id:7").unwrap();
    assert!(!narrows_nothing(
        "archived:1 thread_id:7",
        &paired,
        &plans(&db, &paired, true, None).unwrap().sms
    ));

    // A misspelt name beside a real one costs precision, not the whole table.
    // That is why the guard reads the plan instead of refusing unknown names
    // outright — a messages query carries `trash` and `bucket_id` over from a
    // media view, and refusing those would break search.
    let mixed = provider_plan::fields(&db, "zzz_no_such_field:x ids:1").unwrap();
    assert!(!narrows_nothing(
        "zzz_no_such_field:x ids:1",
        &mixed,
        &plans(&db, &mixed, true, None).unwrap().sms
    ));

    // The two queries allowed to select everything, because they say so.
    for query in ["", "all:true"] {
        let fields = provider_plan::fields(&db, query).unwrap();
        let plans = plans(&db, &fields, true, None).unwrap();
        assert!(plans.sms.clauses.is_empty());
        assert!(!narrows_nothing(query, &fields, &plans.sms));
    }
}

/// The predicate above is only worth anything if `prepare` asks it, and
/// `prepare` is only reached with `destructive` set from the `Ids` arm — the
/// one `trashSms` / `restoreSms` / `deleteSms` resolve their ids through. So
/// this drives the real request type and watches both what the platform is
/// asked for and what comes back.
#[tokio::test]
async fn the_ids_request_refuses_a_whole_table_selection_before_asking_the_platform() {
    fn stub(host: Arc<Host>, ids: Vec<String>) {
        let (generation, mut requests) = host.connect();
        tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let Some(id) = request["id"].as_u64() else {
                    continue;
                };
                let result = match request["method"].as_str().unwrap_or_default() {
                    "systemCapabilities" => json!(["SMS"]),
                    "systemSmsIdsFacts" => json!(ids),
                    other => panic!("unexpected host call {other}"),
                };
                let _ = host.reply(generation, json!({"id": id, "result": result}));
            }
        });
    }

    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    let host = Arc::new(Host::default());
    stub(host.clone(), vec!["1".to_owned(), "2".to_owned()]);

    let error = execute(
        &db,
        &host,
        Request::Ids {
            query: "archived:1".into(),
            include_trashed: true,
        },
    )
    .await
    .expect_err("a query that builds no clause must not reach the platform");
    assert!(
        error.to_string().contains("whole table"),
        "the refusal has to say why, got: {error}"
    );

    // A real selection still resolves, and the platform sees exactly the ids
    // the clause picked out.
    let resolved = execute(
        &db,
        &host,
        Request::Ids {
            query: "ids:7".into(),
            include_trashed: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(resolved, json!({"ids": ["1", "2"]}));
}

#[test]
fn mms_text_search_joins_parts_and_requires_every_filter() {
    let parts = serde_json::json!({"one":["First line", "Second Line"],"two":["First only"]});
    assert_eq!(
        text_ids(parts.clone(), &["first".into(), "LINE".into()]).unwrap(),
        ["one"]
    );
    assert!(
        text_ids(parts, &["first".into(), "missing".into()])
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn conversation_provider_reports_raw_candidates_and_rust_filters_own_numbers() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let host = std::sync::Arc::new(plain_rs::host::bridge::Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let responder = tokio::spawn(async move {
        let request = loop {
            let request = requests.recv().await.unwrap();
            if request["method"] == "systemCapabilities" {
                replies
                    .reply(generation, json!({"id":request["id"],"result":["SMS"]}))
                    .unwrap();
            } else {
                break request;
            }
        };
        assert_eq!(request["method"], "systemSmsConversationFacts");
        replies.reply(generation, serde_json::json!({"id":request["id"],"result":{"items":[{"id":"thread","date":"2026-10-16T00:00:00Z","snippet":"Message","messageCount":1,"read":true,"addresses":["+1 602 555 0100","+1 480 555 0101"]}],"ownNumbers":["6025550100"],"snippetCandidates":{}}})).unwrap();
    });
    let result = execute(
        &db,
        &host,
        Request::Conversations {
            query: String::new(),
            offset: 0,
            limit: 10,
        },
    )
    .await
    .unwrap();
    assert_eq!(result["items"][0]["address"], "+1 480 555 0101");
    assert_eq!(
        result["items"][0]["addresses"],
        serde_json::json!(["+1 480 555 0101"])
    );
    responder.await.unwrap();
    host.disconnect(generation);
}

#[test]
fn mms_page_uses_raw_parts_for_body_attachments_and_address_fallback() {
    let row = serde_json::json!({
        "id":"mms_1", "body":"", "isMms":true, "address":"",
        "date":"2026-10-16T00:00:00Z",
        "addressCandidates":[{"value":"insert-address-token","isSender":true}],
        "parts":[
            {"path":"text","contentType":"text/plain","name":"","fileName":"","hasData":false,"text":" message "},
            {"path":"content://mms/part/2","contentType":"image/jpeg","name":"","fileName":"photo.jpg","hasData":true,"text":""}
        ]
    });
    let items = page(vec![row], 0, 10, "canonical").unwrap();
    assert_eq!(items[0]["body"], "message");
    assert_eq!(items[0]["address"], "canonical");
    assert_eq!(
        items[0]["attachments"],
        serde_json::json!([{"path":"content://mms/part/2","contentType":"image/jpeg","name":"photo.jpg"}])
    );
    assert!(items[0].get("parts").is_none());
    assert!(page(vec![serde_json::json!({"isMms":true,"date":"2026-10-16T00:00:00Z","addressCandidates":[]})], 0, 10, "").is_err());
}

#[test]
fn query_trash_selector_applies_to_sms_and_mms_even_for_internal_all_reads() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(&dir);
    db.trashed_message_save_many(&[
        plain_rs::db::TrashedMessageRow {
            message_id: "1".into(),
            is_mms: false,
            trashed_at: "2026-01-01T00:00:00Z".into(),
        },
        plain_rs::db::TrashedMessageRow {
            message_id: "2".into(),
            is_mms: true,
            trashed_at: "2026-01-01T00:00:00Z".into(),
        },
    ])
    .unwrap();
    for query in ["trash:true", "trashed:1"] {
        let fields = provider_plan::fields(&db, query).unwrap();
        for all in [false, true] {
            let selected = plans(&db, &fields, all, None).unwrap();
            assert!(
                selected
                    .sms
                    .clauses
                    .iter()
                    .any(|clause| clause.contains(" IN "))
            );
            assert!(
                !selected
                    .sms
                    .clauses
                    .iter()
                    .any(|clause| clause.contains("NOT IN"))
            );
            assert!(selected.sms.args.contains(&"1".into()));
            let mms = selected.mms.unwrap();
            assert!(mms.args.contains(&"2".into()));
        }
    }
    for query in ["", "trash:false"] {
        let fields = provider_plan::fields(&db, query).unwrap();
        let active = plans(&db, &fields, false, None).unwrap();
        assert!(
            active
                .sms
                .clauses
                .iter()
                .any(|clause| clause.contains("NOT IN"))
        );
        assert!(active.sms.args.contains(&"1".into()));
        assert!(active.mms.unwrap().args.contains(&"2".into()));
    }
}

#[tokio::test]
async fn filtered_conversation_rows_and_count_decode_the_actual_android_pair_object_facts() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let responder = tokio::spawn(async move {
        let mut calls = Vec::new();
        for index in 0..4 {
            let request = loop {
                let request = requests.recv().await.unwrap();
                if request["method"] == "systemCapabilities" {
                    replies
                        .reply(generation, json!({"id":request["id"],"result":["SMS"]}))
                        .unwrap();
                } else {
                    break request;
                }
            };
            let method = request["method"].as_str().unwrap().to_owned();
            calls.push(method.clone());
            let result = if index % 2 == 0 {
                assert_eq!(method, "systemSmsThreadFacts");
                assert!(
                    request["params"]["plans"]["sms"]["args"]
                        .as_array()
                        .unwrap()
                        .contains(&json!("7"))
                );
                json!([
                    {"first":"7","second":"2026-10-16T00:00:00Z"},
                    {"first":"7","second":"2026-10-16T01:00:00Z"}
                ])
            } else {
                assert_eq!(method, "systemSmsConversationFacts");
                assert_eq!(request["params"]["ids"], json!(["7"]));
                json!({"items":[{"id":"7","date":"2026-10-16T01:00:00Z","snippet":"actual thread","messageCount":2,"read":true,"addresses":["55501001"]}],"ownNumbers":[],"snippetCandidates":{}})
            };
            replies
                .reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
        calls
    });
    let rows = execute(
        &db,
        &host,
        Request::Conversations {
            query: "thread_id:7".into(),
            offset: 0,
            limit: 10,
        },
    )
    .await
    .unwrap();
    assert_eq!(rows["items"].as_array().unwrap().len(), 1);
    assert_eq!(rows["items"][0]["id"], "7");
    assert_eq!(rows["items"][0]["messageCount"], 2);
    let count = execute(
        &db,
        &host,
        Request::ConversationCount {
            query: "thread_id:7".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(count, json!({"count":1}));
    assert_eq!(
        responder.await.unwrap(),
        [
            "systemSmsThreadFacts",
            "systemSmsConversationFacts",
            "systemSmsThreadFacts",
            "systemSmsConversationFacts"
        ]
    );
    host.disconnect(generation);
}
