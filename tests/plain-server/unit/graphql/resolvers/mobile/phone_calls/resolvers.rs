use super::*;
use crate::test_support::schema::MobileFixture as PublicSchema;
use serde_json::{Value, json};

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let method = request["method"].as_str().unwrap_or_default();
            let result = if method == "systemCapabilities" {
                json!(["CALLS", "CALL_PHONE"])
            } else {
                handler(method, request["params"].clone())
            };
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn fixture(permissions: Value) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::test_support::schema::build_mobile(host, events, prefs, db, directory),
    )
}

fn schema_host(schema: &PublicSchema) -> Arc<Host> {
    schema.data::<Arc<Host>>().unwrap().clone()
}

fn call_fact(id: &str, kind: i64) -> Value {
    json!({
        "id": id, "number": "+8618012345678", "name": "Ada", "photoUri": "",
        "startedAt": "2026-10-05T00:00:00.000Z", "durationSec": 42, "type": kind,
        "accountId": "acc-1",
        "geo": {"country":"US","numberType":"MOBILE","carrier":"Test","description":"mobile"},
    })
}

/// The direction code the platform does not recognise reads back as UNKNOWN.
#[test]
fn unknown_call_direction_is_unknown() {
    assert_eq!(CallType::from_android(0), CallType::Unknown);
    assert_eq!(CallType::from_android(99), CallType::Unknown);
    assert_eq!(CallType::from_android(7), CallType::AnsweredExternally);
    assert_eq!(CallType::from_android(3), CallType::Missed);
}

#[tokio::test]
async fn calls_map_the_provider_rows_onto_the_contract_type() {
    let (_dir, schema) = fixture(json!(["READ_CALL_LOG"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemCallFacts" => {
            assert!(params.get("query").is_none());
            assert_eq!(params["plan"]["args"], json!(["ada"]));
            assert_eq!(params["offset"], 0);
            assert_eq!(params["limit"], 20);
            json!([call_fact("c1", 2)])
        }
        "systemPhoneLocaleFacts" => json!({"region":"CN","locale":"zh-CN"}),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"query { calls(offset:0, limit:20, query:"text:ada") {
                 id number name photoId startedAt durationSec type accountId
                 tags { id name count }
                 geo { country numberType carrier description } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let call = &response.data.into_json().unwrap()["calls"][0];
    assert_eq!(call["id"], "c1");
    assert_eq!(call["type"], "OUTGOING");
    assert_eq!(call["durationSec"], 42);
    assert_eq!(call["startedAt"], "2026-10-05T00:00:00.000Z");
    assert_eq!(call["accountId"], "acc-1");
    assert_eq!(call["geo"]["numberType"], "MOBILE");
    assert_eq!(call["geo"]["country"], "CN");
    assert_eq!(call["tags"], json!([]));
}

#[tokio::test]
async fn call_count_degrades_to_zero_without_the_platform_grant() {
    let (_dir, schema) = fixture(json!(["READ_CALL_LOG"]));
    stub(schema_host(&schema), |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_CALL_LOG": false}}),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema.execute(r#"query { callCount(query:"") }"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["callCount"], 0);
}

#[tokio::test]
async fn call_reads_refuse_to_answer_without_the_web_permission() {
    let (_dir, schema) = fixture(json!([]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    let response = schema
        .execute(r#"query { calls(offset:0, limit:1, query:"") { id } }"#)
        .await;
    assert_eq!(response.errors[0].message, "no_permission");
}

#[tokio::test]
async fn placing_a_call_needs_the_call_phone_permission() {
    let (_dir, schema) = fixture(json!(["READ_CALL_LOG"]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    let response = schema
        .execute(r#"mutation { call(number:"+1555", showDialer:false) }"#)
        .await;
    assert_eq!(response.errors[0].message, "no_permission");

    let (_dir, schema) = fixture(json!(["CALL_PHONE"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemMakeCall" => {
            assert_eq!(params["number"], "+1555");
            assert_eq!(params["showDialer"], true);
            json!(true)
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { call(number:"+1555", showDialer:true) }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["call"], true);
}

#[tokio::test]
async fn delete_calls_counts_only_what_the_platform_deleted() {
    let (_dir, schema) = fixture(json!(["WRITE_CALL_LOG"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemCallIds" => {
            assert!(params.get("query").is_none());
            assert_eq!(params["plan"]["clauses"], json!([]));
            json!(["a", "b"])
        }
        "systemDeleteRecords" => {
            assert_eq!(params["provider"], "CALL");
            assert_eq!(params["ids"], json!(["a", "b"]));
            json!(["b"])
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { deleteCalls(query:"all:true") { affectedCount } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteCalls"]["affectedCount"],
        1
    );
}

#[tokio::test]
async fn delete_calls_rejects_a_blank_query_before_touching_the_platform() {
    let (_dir, schema) = fixture(json!(["WRITE_CALL_LOG"]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    let response = schema
        .execute(r#"mutation { deleteCalls(query:" ") { affectedCount } }"#)
        .await;
    assert!(
        response.errors[0]
            .message
            .starts_with("query is required for bulk mutations"),
        "{:?}",
        response.errors
    );
}

/// Call contributes no clause of its own — it has no trash scope and no
/// duration floor — so every field the web client carries into it from a media
/// view builds nothing, and an empty clause list is `1=1` on the way to the
/// platform. That is the whole call log.
#[tokio::test]
async fn delete_calls_refuses_a_query_of_fields_it_does_not_act_on() {
    let (_dir, schema) = fixture(json!(["WRITE_CALL_LOG"]));
    stub(schema_host(&schema), |method, _| {
        panic!("a refused delete must not reach the platform: {method}")
    });
    for query in ["trash:false", "show_hidden:false"] {
        let response = schema
            .execute(&format!(
                r#"mutation {{ deleteCalls(query:"{query}") {{ affectedCount }} }}"#
            ))
            .await;
        assert_eq!(response.errors.len(), 1, "{query} was not refused");
        assert!(
            response.errors[0].message.contains("selects no"),
            "{query} failed for the wrong reason: {}",
            response.errors[0].message
        );
    }
}

#[tokio::test]
async fn calls_without_geo_preserve_provider_values_without_phone_metadata_calls() {
    let (_dir, schema) = fixture(json!(["READ_CALL_LOG"]));
    let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let observed = calls.clone();
    stub(schema_host(&schema), move |method, _| {
        observed.lock().unwrap().push(method.to_owned());
        match method {
            "systemCallFacts" => {
                let mut fact = call_fact("1", 37);
                fact["photoUri"] = json!("content://contacts/photo/1");
                json!([fact])
            }
            other => panic!("unexpected host call {other}"),
        }
    });
    let response = schema
        .execute(r#"{ calls(query:"", offset:0, limit:10) { id type typeCode photoUri } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["calls"][0]["type"], "UNKNOWN");
    assert_eq!(data["calls"][0]["typeCode"], 37);
    assert_eq!(data["calls"][0]["photoUri"], "content://contacts/photo/1");
    assert_eq!(*calls.lock().unwrap(), ["systemCallFacts"]);
}

#[tokio::test]
async fn phone_geo_samples_locale_once_for_the_page_and_never_dispatches_number_metadata() {
    let (_dir, schema) = fixture(json!(["READ_CALL_LOG"]));
    let requests = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let observed = requests.clone();
    stub(schema_host(&schema), move |method, _| {
        observed.lock().unwrap().push(method.into());
        match method {
            "systemCallFacts" => json!(
                (0..128)
                    .map(|index| {
                        let mut fact = call_fact(&index.to_string(), 1);
                        fact["number"] = json!(format!("800-555-0100 ext {index}"));
                        fact
                    })
                    .collect::<Vec<_>>()
            ),
            "systemPhoneLocaleFacts" => json!({"region":"US","locale":"en-US"}),
            other => panic!("unexpected host call {other}"),
        }
    });
    let response = schema.execute(r#"{ calls(query:"", offset:0, limit:128) { id geo { country numberType carrier description } } }"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let rows = data["calls"].as_array().unwrap();
    assert_eq!(rows.len(), 128);
    assert!(
        rows.iter()
            .all(|row| row["geo"]["country"] == "US" && row["geo"]["numberType"] == "TOLL_FREE")
    );
    assert_eq!(
        *requests.lock().unwrap(),
        ["systemCallFacts", "systemPhoneLocaleFacts"]
    );
}

#[tokio::test]
async fn empty_call_page_does_not_request_locale_or_number_metadata() {
    let (_dir, schema) = fixture(json!(["READ_CALL_LOG"]));
    stub(schema_host(&schema), |method, _| match method {
        "systemCallFacts" => json!([]),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"{ calls(query:"", offset:0, limit:20) { geo { country } } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["calls"], json!([]));
}
