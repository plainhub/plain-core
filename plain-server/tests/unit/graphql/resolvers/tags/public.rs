use super::*;
use crate::graphql::schemas::public::PublicSchema;
use plain_rs::db::Db;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = handler(
                &request["method"].as_str().unwrap_or_default(),
                request["params"].clone(),
            );
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

/// Any host call is a bug for these tests: the whole point is that tags are
/// Rust SQLite rows, so nothing should reach the platform.
fn fixture() -> (tempfile::TempDir, PublicSchema) {
    fixture_with(|method, _| panic!("unexpected host call {method}"))
}

fn fixture_with(
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    stub(host.clone(), handler);
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::graphql::schemas::public::build(host, events, prefs, db, directory),
    )
}

fn database(schema: &PublicSchema) -> &Arc<Db> {
    schema.data::<Arc<Db>>().unwrap()
}

fn doc(path: &str) -> plain_rs::db::MediaFileMetadata {
    plain_rs::db::MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type: DataType::Doc,
        path: path.to_owned(),
        title: std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        artist: String::new(),
        bucket_id: "b3".to_owned(),
        size_bytes: 10,
        duration_ms: 0,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-02T03:04:05.000Z".to_owned(),
        updated_at: "2026-01-02T03:04:05.000Z".to_owned(),
    }
}

/// The ordinals are plain-app's `DataType.value`, stored in the tag tables.
/// They are not a free ordering — changing one would re-file every existing
/// tag, so the contract enum and this mapping are pinned.
#[test]
fn content_kinds_keep_plain_apps_ordinals() {
    assert_eq!(DataType::Default.kind(), 0);
    assert_eq!(DataType::Audio.kind(), 1);
    assert_eq!(DataType::Video.kind(), 2);
    assert_eq!(DataType::Image.kind(), 3);
    assert_eq!(DataType::Sms.kind(), 4);
    assert_eq!(DataType::Contact.kind(), 5);
    assert_eq!(DataType::Note.kind(), 6);
    assert_eq!(DataType::FeedEntry.kind(), 7);
    assert_eq!(DataType::Call.kind(), 8);
    assert_eq!(DataType::Package.kind(), 21);
    assert_eq!(DataType::File.kind(), 22);
    assert_eq!(DataType::AppFile.kind(), 23);
    assert_eq!(DataType::Doc.kind(), 24);
}

#[tokio::test]
async fn a_created_tag_reads_back_with_its_kind_implied_not_stored() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation { createTag(type:IMAGE, name:"holiday") { id name count } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let created = response.data.into_json().unwrap();
    let id = created["createTag"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["createTag"]["name"], "holiday");
    assert_eq!(created["createTag"]["count"], 0);

    let response = schema
        .execute(&format!(
            r#"query {{ tags(type:IMAGE) {{ id name count }} audio: tags(type:AUDIO) {{ id }} }}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["tags"].as_array().unwrap().len(), 1);
    assert_eq!(data["tags"][0]["id"], id.as_str());
    // The kind filter is real: an audio list does not see an image tag.
    assert_eq!(data["audio"], json!([]));
}

#[tokio::test]
async fn an_absent_tag_is_omitted_from_the_existing_typed_list() {
    let (_dir, schema) = fixture();
    let response = schema.execute("{tags(type:IMAGE){id}}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["tags"], json!([]));
}

#[tokio::test]
async fn updating_an_unknown_tag_errors_instead_of_inventing_one() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation { updateTag(id:"nope", name:"x") { id } }"#)
        .await;
    assert_eq!(response.errors.len(), 1);
    assert!(response.errors[0].message.contains("not found"));
}

#[tokio::test]
async fn deleting_a_tag_drops_its_relations_too() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation { createTag(type:DOC, name:"pdf") { id } }"#)
        .await;
    let id = response.data.into_json().unwrap()["createTag"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = schema
        .execute(&format!(
            r#"mutation {{ updateTagRelations(type:DOC, keys:["d1"],
                              addTagIds:["{id}"], removeTagIds:[]) }}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let response = schema.execute(r#"query { tagKeys(id:"IGNORED") } "#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    let response = schema
        .execute(&format!(r#"mutation {{ deleteTag(id:"{id}") }}"#))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let response = schema
        .execute(&format!(r#"query {{ keys: tagKeys(id:"{id}") }}"#))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["keys"], json!([]));
}

#[tokio::test]
async fn relations_are_listed_for_the_requested_keys_and_kind() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(
            r#"mutation { a: createTag(type:VIDEO, name:"v") { id }
                             b: createTag(type:VIDEO, name:"w") { id } }"#,
        )
        .await;
    let data = response.data.into_json().unwrap();
    let a = data["a"]["id"].as_str().unwrap().to_string();
    let b = data["b"]["id"].as_str().unwrap().to_string();
    let response = schema
        .execute(&format!(
            r#"mutation {{ one: updateTagRelations(type:VIDEO, keys:["k1"],
                                    addTagIds:["{a}"], removeTagIds:[])
                              two: updateTagRelations(type:VIDEO, keys:["k2"],
                                    addTagIds:["{a}"], removeTagIds:[])
                              rel: updateTagRelations(type:VIDEO, keys:["k1"],
                                    addTagIds:["{b}"], removeTagIds:["{a}"]) }}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    let response = schema
        .execute(r#"query { tagRelations(type:VIDEO, keys:["k1","k2","k3"]) { tagId key } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let rows = response.data.into_json().unwrap();
    let rows = rows["tagRelations"].as_array().unwrap();
    // k3 was never touched, so it contributes nothing.
    assert_eq!(rows.len(), 2);
    // The last mutation moved k1 from a to b; k2 still has a.
    let k1 = rows
        .iter()
        .find(|row| row["key"] == "k1")
        .expect("k1 relation");
    let k2 = rows
        .iter()
        .find(|row| row["key"] == "k2")
        .expect("k2 relation");
    assert_eq!(k1["tagId"], b.as_str());
    assert_eq!(k2["tagId"], a.as_str());
}

/// A relation may only point at a tag of the same kind — the library
/// rejects a mismatch rather than filing an image tag under a video.
#[tokio::test]
async fn a_relation_cannot_cross_kinds() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation { createTag(type:IMAGE, name:"i") { id } }"#)
        .await;
    let id = response.data.into_json().unwrap()["createTag"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = schema
        .execute(&format!(
            r#"mutation {{ updateTagRelations(type:VIDEO, keys:["k1"],
                              addTagIds:["{id}"], removeTagIds:[]) }}"#
        ))
        .await;
    assert_eq!(response.errors.len(), 1);
    assert!(response.errors[0].message.contains("tag type mismatch"));
    let response = schema
        .execute(r#"query { tagRelations(type:VIDEO, keys:["k1"]) { key } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["tagRelations"],
        json!([])
    );
}

#[tokio::test]
async fn adding_a_tag_to_a_query_attaches_it_to_every_match() {
    let (_dir, schema) = fixture();
    let stored = database(&schema)
        .record_media_files(&[
            doc("/storage/Docs/report-a.pdf"),
            doc("/storage/Docs/report-b.pdf"),
        ])
        .unwrap();
    let response = schema
        .execute(r#"mutation { createTag(type:DOC, name:"work") { id } }"#)
        .await;
    let id = response.data.into_json().unwrap()["createTag"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = schema
        .execute(&format!(
            r#"mutation {{ addToTags(type:DOC, tagIds:["{id}"], query:"text:report") }}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let response = schema
        .execute(&format!(r#"query {{ tagKeys(id:"{id}") }}"#))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let keys = response.data.into_json().unwrap()["tagKeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        keys,
        stored
            .iter()
            .map(|row| row.id.clone())
            .collect::<std::collections::HashSet<_>>()
    );
}

#[tokio::test]
async fn removing_a_tag_takes_it_off_every_match() {
    let (_dir, schema) = fixture();
    let stored = database(&schema)
        .record_media_files(&[doc("/storage/Docs/x1.pdf"), doc("/storage/Docs/x2.pdf")])
        .unwrap();
    let response = schema
        .execute(r#"mutation { createTag(type:DOC, name:"work") { id } }"#)
        .await;
    let id = response.data.into_json().unwrap()["createTag"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for row in &stored {
        let query = format!(
            r#"mutation {{ updateTagRelations(type:DOC, keys:["{}"], addTagIds:["{id}"], removeTagIds:[]) }}"#,
            row.id
        );
        let response = schema.execute(&query).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
    }
    let response = schema
        .execute(&format!(
            r#"mutation {{ removeFromTags(type:DOC, tagIds:["{id}"], query:"text:x") }}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let response = schema
        .execute(&format!(r#"query {{ tagKeys(id:"{id}") }}"#))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["tagKeys"], json!([]));
}

/// A blank query matches every row of the kind. Refusing it is the only
/// thing standing between a client bug and tagging the whole library.
#[tokio::test]
async fn a_blank_query_is_refused_before_the_platform_is_touched() {
    let (_dir, schema) =
        fixture_with(|method, _| panic!("the platform must not be asked: {method}"));
    let response = schema
        .execute(r#"mutation { createTag(type:DOC, name:"work") { id } }"#)
        .await;
    let id = response.data.into_json().unwrap()["createTag"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for document in [
        format!(r#"mutation {{ addToTags(type:DOC, tagIds:["{id}"], query:"") }}"#),
        format!(r#"mutation {{ removeFromTags(type:DOC, tagIds:["{id}"], query:"   ") }}"#),
    ] {
        let response = schema.execute(&document).await;
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, "explicit query required");
    }
}

#[tokio::test]
async fn notes_and_feed_entries_tag_selected_ids_without_host_or_body_reads() {
    use async_graphql::{EmptySubscription, Schema};
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let now = "2026-01-02T03:04:05.000Z";
    for id in ["selected", "neighbor"] {
        db.note_save(&format!("note-{id}"), id, "body", now)
            .unwrap();
        db.with_conn(|connection| connection.execute(
            "INSERT INTO feed_entries(id,title,url,image,description,author,content,feed_id,source_entry_id,published_at,is_read,created_at,updated_at) VALUES(?1,?1,'https://example.invalid/','','','','body','feed',?1,?2,0,?2,?2)",
            rusqlite::params![format!("feed-{id}"), now],
        )).unwrap();
    }
    db.with_conn(|connection| {
        connection
            .authorizer(Some(|context: AuthContext<'_>| match context.action {
                AuthAction::Read {
                    table_name: "notes" | "feed_entries",
                    column_name,
                } if !["id", "deleted_at"].contains(&column_name) => Authorization::Deny,
                _ => Authorization::Allow,
            }))
            .unwrap()
    });
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let schema = Schema::build(TagsQuery, TagsMutation, EmptySubscription)
        .data(db.clone())
        .data(host.clone())
        .finish();
    for (kind, name, selected) in [
        (DataType::Note, "NOTE", "note-selected"),
        (DataType::FeedEntry, "FEED_ENTRY", "feed-selected"),
    ] {
        let tag = tags::create_tag(&db, kind.kind(), name).unwrap();
        for action in ["addToTags", "removeFromTags"] {
            if action == "removeFromTags" {
                assert_eq!(tags::keys_for_tag(&db, &tag.id).unwrap(), [selected]);
            }
            for query in [
                "".to_owned(),
                " \t\n".to_owned(),
                "all:false".to_owned(),
                "unsupported_scope:value".to_owned(),
                format!("ids:{selected}"),
            ] {
                let request = async_graphql::Request::new(format!(
                    "mutation($q:String!){{ {action}(type:{name},tagIds:[\"{}\"],query:$q) }}",
                    tag.id
                ))
                .variables(async_graphql::Variables::from_json(json!({"q":&query})));
                let response = tokio::select! {
                    response = schema.execute(request) => response,
                    request = requests.recv() => panic!("unexpected host request {request:?}"),
                };
                if query == format!("ids:{selected}") {
                    assert!(response.errors.is_empty(), "{:?}", response.errors);
                } else {
                    assert!(
                        !response.errors.is_empty(),
                        "accepted unsafe scope {query:?}"
                    );
                    let expected = if action == "removeFromTags" {
                        vec![selected.to_owned()]
                    } else {
                        vec![]
                    };
                    assert_eq!(tags::keys_for_tag(&db, &tag.id).unwrap(), expected);
                }
            }
        }
        assert!(tags::keys_for_tag(&db, &tag.id).unwrap().is_empty());
    }
    assert!(requests.try_recv().is_err());
    host.disconnect(generation);
}

#[tokio::test]
async fn the_contract_tag_type_carries_no_numeric_kind() {
    // The list it came from implies the kind, so a `type` field would be a
    // second, disagreeing source for the same fact.
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation { createTag(type:IMAGE, name:"x") { id name count } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let tag = &data["createTag"];
    assert!(tag.get("type").is_none(), "{tag}");
}
