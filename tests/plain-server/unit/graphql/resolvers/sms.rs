use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_libs::serde_json::{Value, json};

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
                json!(["TELEPHONY"])
            } else {
                handler(method, request["params"].clone())
            };
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn fixture(permissions: Value) -> (plain_libs::temporary::TempDir, PublicSchema) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::test_support::schema::build(host, events, prefs, db, directory),
    )
}

fn schema_host(schema: &PublicSchema) -> Arc<Host> {
    schema.data::<Arc<Host>>().unwrap().clone()
}

fn sms_fact(id: &str) -> Value {
    json!({
        "id": id, "body": "hi", "address": "+1555", "date": "2026-10-05T00:00:00.000Z",
        "serviceCenter": "+1999", "read": true, "threadId": "t1", "type": 1,
        "subscriptionId": 2, "isMms": false,
        "attachments": [{"path":"/a.png","contentType":"image/png","name":"a.png"}],
    })
}

/// Everything the SMS roots need before a query reaches the platform.
fn sms_facts(schema: &PublicSchema, items: Value, count: i64) {
    stub(schema_host(schema), move |method, _| match method {
        "systemAccessFacts" => json!({"granted":{"SMS": true}}),
        "systemSmsRowsFacts" => {
            json!({"items": items.clone(), "canonicalAddresses": ["+1555"], "ownNumbers": []})
        }
        "systemSmsCountFacts" => json!({"sms": count, "mms": 0}),
        other => panic!("unexpected host call {other}"),
    });
}

#[tokio::test]
async fn sms_reads_degrade_to_empty_without_the_platform_grant() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, _| match method {
        "systemAccessFacts" => json!({"granted":{"SMS": false}}),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"query { sms(offset:0, limit:10, query:"") { id }
                 smsCount(query:"") smsConversationCount(query:"")
                 smsBoxCounts { total inbox sent drafts } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["sms"], json!([]));
    assert_eq!(data["smsCount"], 0);
    assert_eq!(data["smsConversationCount"], 0);
    assert_eq!(
        data["smsBoxCounts"],
        json!({"total":0,"inbox":0,"sent":0,"drafts":0})
    );
}

#[tokio::test]
async fn sms_maps_the_provider_rows_onto_the_contract_type() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    sms_facts(&schema, json!([sms_fact("m1")]), 3);
    let response = schema
        .execute(
            r#"query { sms(offset:0, limit:10, query:"") {
                 id body address sentAt serviceCenter read threadId type
                 subscriptionId isMms tags { id name count }
                 attachments { path contentType name } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let message = &response.data.into_json().unwrap()["sms"][0];
    assert_eq!(message["id"], "m1");
    assert_eq!(message["type"], "INBOX");
    assert_eq!(message["sentAt"], "2026-10-05T00:00:00.000Z");
    assert_eq!(message["threadId"], "t1");
    assert_eq!(message["serviceCenter"], "+1999");
    assert_eq!(message["attachments"][0]["contentType"], "image/png");
    assert_eq!(message["tags"], json!([]));

    let response = schema.execute(r#"query { smsCount(query:"") }"#).await;
    assert_eq!(response.data.into_json().unwrap()["smsCount"], 3);
}

/// An unknown provider box must read back as UNKNOWN, not panic and not
/// silently become INBOX.
#[test]
fn unknown_sms_box_is_unknown() {
    assert_eq!(SmsType::from_android(0), SmsType::Unknown);
    assert_eq!(SmsType::from_android(99), SmsType::Unknown);
    assert_eq!(SmsType::from_android(6), SmsType::Queued);
}

#[tokio::test]
async fn bulk_sms_mutations_reject_a_blank_query_before_touching_the_platform() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    for mutation in ["trashSms", "restoreSms", "deleteSms"] {
        let response = schema
            .execute(&format!(
                r#"mutation {{ {mutation}(query:"  ") {{ affectedCount }} }}"#
            ))
            .await;
        assert!(
            response.errors[0]
                .message
                .starts_with("query is required for bulk mutations"),
            "{mutation}: {:?}",
            response.errors
        );
    }
}

#[tokio::test]
async fn bulk_sms_mutations_select_ids_in_rust_and_delete_only_provider_rows() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemCapabilities" => json!(["TELEPHONY"]),
        "systemSmsIdsFacts" => {
            assert!(params.get("query").is_none());
            assert!(params.get("sms").is_some());
            json!(["1", "2"])
        }
        "systemSmsRowsFacts" => {
            json!({"items":[sms_fact("1"), sms_fact("2")],"canonicalAddresses":[],"ownNumbers":[]})
        }
        "systemDeleteSmsRows" => {
            assert_eq!(params, json!({"ids":["1", "2"]}));
            json!({"deletedIds":["1", "2"],"failedIds":[]})
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { trashSms(query:"ids:1,2") { affectedCount } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["trashSms"]["affectedCount"],
        2
    );
    assert_eq!(
        schema
            .data::<Arc<Db>>()
            .unwrap()
            .trashed_message_ids()
            .unwrap()
            .len(),
        2
    );
    let response = schema
        .execute(r#"mutation { deleteSms(query:"all:true") { affectedCount } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteSms"]["affectedCount"],
        2
    );
}

#[tokio::test]
async fn archiving_a_conversation_stores_the_last_message_date() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, _| match method {
        "systemCapabilities" => json!(["TELEPHONY"]),
        "systemSmsConversationFacts" => json!({"items":[]}),
        "systemAccessFacts" => json!({"granted":{"SMS": true}}),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { archiveSmsConversation(id:"t1") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["archiveSmsConversation"],
        true
    );
    // The conversation existed with no dated row, so today's date is stored.
    let db = schema.data::<Arc<Db>>().unwrap();
    let records = db.archived_conversation_list().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].conversation_id, "t1");
    let stored = plain_libs::chrono::DateTime::parse_from_rfc3339(&records[0].archived_at).unwrap();
    assert!(
        plain_libs::chrono::Utc::now()
            .signed_duration_since(stored)
            .num_seconds()
            .abs()
            < 120,
        "stored {} is not 'now'",
        records[0].archived_at
    );
}

#[tokio::test]
async fn send_sms_needs_the_send_permission_and_passes_the_sim_choice() {
    let (_dir, schema) = fixture(json!([]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    let response = schema
        .execute(
            r#"mutation { sendSms(number:"+1555", body:"hi", subscriptionId:2, requestId:"11111111-1111-4111-8111-111111111111") { requestId requiresConfirmation } }"#,
        )
        .await;
    assert_eq!(response.errors[0].message, "no_permission");

    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemAccessFacts" => json!({"granted":{"SMS": true}}),
        "systemCapabilities" => json!(["TELEPHONY"]),
        "systemDivideSmsText" => json!(["hi"]),
        "systemSendSmsParts" => {
            assert_eq!(params["number"], "+1555");
            assert_eq!(params["parts"], json!(["hi"]));
            assert_eq!(params["subscriptionId"], 2);
            assert!(!params["requestId"].as_str().unwrap().is_empty());
            assert!(params.get("clientRequestId").is_none());
            json!(true)
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"mutation { sendSms(number:"+1555", body:"hi", subscriptionId:2, requestId:"11111111-1111-4111-8111-111111111111") { requestId requiresConfirmation } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["sendSms"],
        json!({"requestId":"11111111-1111-4111-8111-111111111111","requiresConfirmation":false})
    );
}

/// -1 means "system default", which reaches the platform as no subscription.
#[tokio::test]
async fn send_sms_maps_the_default_sim_to_no_subscription() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemCapabilities" => json!(["TELEPHONY"]),
        "systemDivideSmsText" => json!(["hi"]),
        "systemSendSmsParts" => {
            assert_eq!(params["subscriptionId"], Value::Null);
            assert_eq!(params["requestId"], "22222222-2222-4222-8222-222222222222");
            json!(true)
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"mutation { sendSms(number:"+1555", body:"hi", subscriptionId:-1, requestId:"22222222-2222-4222-8222-222222222222") { requestId requiresConfirmation } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

#[tokio::test(start_paused = true)]
async fn send_sms_with_attachments_launches_once_and_replays_the_same_request_result() {
    for success in [false, true] {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (dir, schema) = fixture(json!(["SMS"]));
        let attachment = dir.path().join("synthetic.png");
        std::fs::write(&attachment, b"synthetic MMS attachment").unwrap();
        let launches = Arc::new(AtomicUsize::new(0));
        let counted = launches.clone();
        stub(schema_host(&schema), move |method, params| match method {
            "systemMmsLatest" => json!(7),
            "systemMmsLaunch" => {
                counted.fetch_add(1, Ordering::SeqCst);
                assert_eq!(params["number"], "+1555");
                assert_eq!(params["attachments"].as_array().unwrap().len(), 1);
                assert_eq!(
                    std::fs::read(params["attachments"][0]["path"].as_str().unwrap()).unwrap(),
                    b"synthetic MMS attachment"
                );
                assert!(params.get("threadId").is_none());
                json!(100)
            }
            "systemMmsCandidates" => {
                if success {
                    json!([{"id":8,"addressCandidates":[{"value":"+1555","isSender":false}],"parts":[
                        {"path":"text","contentType":"text/plain","name":"","fileName":"","hasData":false,"text":"hi"},
                        {"path":"image","contentType":"image/png","name":"synthetic.png","fileName":"synthetic.png","hasData":true,"text":""}
                    ]}])
                } else {
                    json!([])
                }
            }
            other => panic!("Unexpected platform primitive: {other}"),
        });
        let mutation = format!(
            r#"mutation {{sendSms(number:"+1555",body:"hi",attachmentPaths:[{}],requestId:"33333333-3333-4333-8333-333333333333") {{ requestId requiresConfirmation }} }}"#,
            json!(attachment)
        );
        for _ in 0..2 {
            let response = schema.execute(&mutation).await;
            assert!(response.errors.is_empty(), "{:?}", response.errors);
            assert_eq!(
                response.data.into_json().unwrap()["sendSms"],
                json!({"requestId":"33333333-3333-4333-8333-333333333333","requiresConfirmation":true})
            );
        }
        assert_eq!(launches.load(Ordering::SeqCst), 1);
        let conflict = schema
            .execute(mutation.replace("body:\"hi\"", "body:\"changed\""))
            .await;
        assert!(conflict.errors[0].message.contains("different message"));
        assert_eq!(launches.load(Ordering::SeqCst), 1);
        let temporary = dir.path().join("mms_attachments");
        let directories: Vec<_> = std::fs::read_dir(&temporary)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(directories.len(), 1);
        assert!(directories[0].starts_with(&temporary));
        assert_eq!(
            directories[0].file_name().unwrap().to_str().unwrap().len(),
            36
        );
        let runtime = schema
            .data::<Arc<plain_rs::features::services::sms::mms::Runtime>>()
            .unwrap();
        let mut events = schema
            .data::<tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>>()
            .unwrap()
            .subscribe();
        if !success {
            runtime.cancel_all().await;
        }
        let event = tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.event_type, "SMS_SEND_RESULT");
        assert_eq!(
            plain_libs::serde_json::from_str::<Value>(&event.payload).unwrap(),
            json!({"requestId":"33333333-3333-4333-8333-333333333333","success":success,"resultCode":if success {0} else {-1001}})
        );
        runtime.replay().unwrap();
        let replay = events.recv().await.unwrap();
        assert_eq!(replay.event_type, event.event_type);
        assert_eq!(replay.payload, event.payload);
        runtime.shutdown().await;
        assert!(!directories[0].exists());
        assert_eq!(
            std::fs::read(attachment).unwrap(),
            b"synthetic MMS attachment"
        );
    }
}

#[tokio::test]
async fn send_sms_preserves_the_client_uuid_with_default_sim() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemDivideSmsText" => json!(["hi"]),
        "systemSendSmsParts" => {
            assert_eq!(params["subscriptionId"], Value::Null);
            assert_eq!(params["requestId"], "44444444-4444-4444-8444-444444444444");
            json!(true)
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"mutation { sendSms(number:"+1555",body:"hi",requestId:"44444444-4444-4444-8444-444444444444") { requestId requiresConfirmation } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let id = data["sendSms"]["requestId"].as_str().unwrap();
    assert_eq!(id, "44444444-4444-4444-8444-444444444444");
    assert_eq!(data["sendSms"]["requiresConfirmation"], false);
    let sends = schema
        .data::<Arc<plain_rs::features::services::sms::send_runtime::Runtime>>()
        .unwrap();
    let pending = plain_rs::features::sms_sends::pending(&sends.store().unwrap()).unwrap();
    assert_eq!(pending[0].request_id, id);
}

#[tokio::test]
async fn send_sms_rejects_mms_sim_selection_and_blank_request_ids_before_effects() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    for (args, expected) in [
        (
            r#"attachmentPaths:["synthetic.txt"],subscriptionId:2,requestId:"55555555-5555-4555-8555-555555555555""#,
            "cannot select a SIM for MMS",
        ),
        (r#"requestId:" ""#, "requestId must be a UUID"),
        (r#"requestId:"../request""#, "requestId must be a UUID"),
    ] {
        let response = schema
            .execute(format!(
                r#"mutation {{ sendSms(number:"+1555",body:"hi",{args}) {{ requestId }} }}"#
            ))
            .await;
        assert!(
            response.errors[0].message.contains(expected),
            "{:?}",
            response.errors
        );
    }
}

#[test]
fn message_projection_preserves_raw_provider_type_for_app_clients() {
    let sms = message(&json!({"id":"7", "type":37, "date":"2026-10-01T00:00:00.000Z"}));
    assert_eq!(sms.type_code, 37);
    assert_eq!(sms.r#type, SmsType::Unknown);
}

#[tokio::test]
async fn send_sms_requires_a_request_id_before_platform_effects() {
    let (_dir, schema) = fixture(json!(["SMS"]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    for argument in ["", ",requestId:null"] {
        let response = schema
            .execute(format!(
                r#"mutation {{sendSms(number:"+1555",body:"hi"{argument}) {{requestId}}}}"#
            ))
            .await;
        assert!(!response.errors.is_empty());
    }
}
