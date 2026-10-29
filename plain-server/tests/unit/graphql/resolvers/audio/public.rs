use super::*;
use crate::graphql::schemas::public::PublicSchema;
use crate::host::bridge::Host;
use plain_rs::db::Db;
use plain_rs::media::play_history;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

fn stub(host: Arc<Host>, db: Arc<Db>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = if request["method"] == "audioLibraryRefresh" {
                Value::Null
            } else if request["method"] == "audioLibraryResolve" {
                let params = &request["params"];
                let mut rows = db.with_conn(|conn|conn.prepare("SELECT id,path,title,artist,duration_ms,cover_image_id FROM file_catalog WHERE data_type=1 UNION SELECT file_id,path,title,artist,duration_ms,album_id FROM platform_audio_media UNION SELECT id,path,title,artist,duration_ms,'' FROM file_catalog WHERE data_type=1").unwrap().query_map([],|row|Ok(json!({"mediaId":row.get::<_,String>(0)?,"path":row.get::<_,String>(1)?,"title":row.get::<_,String>(2)?,"artist":row.get::<_,String>(3)?,"durationMs":row.get::<_,i64>(4)?,"albumId":row.get::<_,String>(5)?,"aliases":[]}))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap());
                let mut paths = params["paths"].as_array().unwrap().clone();
                if let Some(ids) = params["ids"].as_array() {
                    paths.extend(ids.iter().filter_map(|id| {
                        id.as_str()
                            .and_then(|id| id.strip_prefix("mock:"))
                            .map(|path| json!(path))
                    }));
                }
                for path in &paths {
                    if rows.iter().any(|row| row["path"] == *path) {
                        continue;
                    }
                    let facts = handler("syntheticAudioTracks", json!({"paths":[path]}));
                    for mut track in facts.as_array().unwrap().iter().cloned() {
                        track["mediaId"] =
                            json!(format!("mock:{}", track["path"].as_str().unwrap()));
                        track["albumId"] = json!("");
                        track["aliases"] = json!([]);
                        rows.push(track);
                    }
                }
                json!(rows)
            } else {
                handler(
                    &request["method"].as_str().unwrap_or_default(),
                    request["params"].clone(),
                )
            };
            let result = if request["method"] == "systemMediaRows"
                && request["params"]["includeRevision"] == true
            {
                json!({"rows":result,"revisionBefore":null,"revisionAfter":null})
            } else {
                result
            };
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn engine_report(params: &Value) -> Value {
    let mut report = params["playback"].clone();
    report["loaded"] = json!(params["action"] == "PLAY");
    report
}

const GRANTED: &str = r#"["WRITE_EXTERNAL_STORAGE"]"#;

fn fixture() -> (tempfile::TempDir, PublicSchema) {
    fixture_with(GRANTED, |method, params| match method {
        "syntheticAudioTracks" => json!(
            params["paths"]
                .as_array()
                .unwrap()
                .iter()
                .map(|path| json!({
                    "title": "Track", "artist": "Artist",
                    "path": path.as_str().unwrap(), "durationMs": 1000,
                }))
                .collect::<Vec<_>>()
        ),
        "systemAudioPlaybackState" => json!({ "isPlaying": false, "positionMs": 0 }),
        "audioEngineCommand" => engine_report(&params),
        "systemMediaRows" => json!([]),
        "systemMediaCount" => json!(0),
        _other => panic!("unexpected host call {_other}"),
    })
}

fn fixture_with(
    permissions: &str,
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set(
            "api_permissions",
            serde_json::from_str::<Vec<String>>(permissions).unwrap(),
        )
        .unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    stub(host.clone(), db.clone(), handler);
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

async fn query(schema: &PublicSchema, document: &str) -> Value {
    let request =
        async_graphql::Request::new(document).data(crate::graphql::executor::RequestOrigin::Remote);
    serde_json::to_value(schema.execute(request).await).unwrap()
}

async fn local_query(schema: &PublicSchema, document: &str) -> Value {
    let request =
        async_graphql::Request::new(document).data(crate::graphql::executor::RequestOrigin::Local);
    serde_json::to_value(schema.execute(request).await).unwrap()
}

fn track(path: &str, title: &str, artist: &str) -> AudioTrack {
    AudioTrack {
        file_id: None,
        path: path.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        album_id: String::new(),
        duration_ms: 1000,
    }
}

fn seed_queue(schema: &PublicSchema, tracks: &[(&str, &str, &str)]) {
    let items: Vec<AudioTrack> = tracks
        .iter()
        .map(|(path, title, artist)| track(path, title, artist))
        .collect();
    for item in &items {
        database(schema).with_conn(|conn|conn.execute("INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES(?1,?2,?3,?4,?5,'')",rusqlite::params![format!("test:{}",item.path),item.path,item.title,item.artist,item.duration_ms])).unwrap();
    }
    domain::enqueue(database(schema), &items, false).unwrap();
}

#[tokio::test]
async fn playlist_query_id_selection_preserves_input_order_and_fails_without_partial_inserts() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "A", "Artist"),
            ("/music/b.mp3", "B", "Artist"),
        ],
    );
    let db = database(&schema);
    let playlist = domain::create_playlist(db, "ordered").unwrap();
    let add = query(&schema, &format!(r#"mutation {{ addAudioPlaylistItems(id:"{}",query:"ids:test:/music/b.mp3,test:/music/a.mp3,test:/music/b.mp3") }}"#, playlist.id)).await;
    assert_eq!(add["data"]["addAudioPlaylistItems"], true, "{add}");
    let rows = domain::playlist_items_page(db, &playlist.id, 0, 10, "").unwrap();
    assert_eq!(
        rows.iter().map(|row| row.path.as_str()).collect::<Vec<_>>(),
        ["/music/b.mp3", "/music/a.mp3"]
    );
    let empty = domain::create_playlist(db, "empty").unwrap();
    for selection in [
        "ids:test:/music/b.mp3,test:/missing",
        "ids:",
        "ids:test:/music/a.mp3,",
    ] {
        let response = query(
            &schema,
            &format!(
                r#"mutation {{ addAudioPlaylistItems(id:"{}",query:{}) }}"#,
                empty.id,
                json!(selection)
            ),
        )
        .await;
        assert!(response.get("errors").is_some(), "{response}");
        assert!(
            domain::playlist_items_page(db, &empty.id, 0, 10, "")
                .unwrap()
                .is_empty()
        );
    }
}

/// A queue seeded behind the resolver's back still reads back through it:
/// that is the point of mounting the app's own service rather than a second
/// copy of the queue.
#[tokio::test]
async fn the_queue_is_paged_and_its_count_matches() {
    let (_dir, schema) = fixture();
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "Alpha", "One"),
            ("/music/b.mp3", "Beta", "Two"),
            ("/music/c.mp3", "Gamma", "Three"),
        ],
    );

    let page = query(
        &schema,
        r#"query { audioQueueItems(offset: 0, limit: 2, query: "") { title artist path durationMs } }"#,
    )
    .await;
    let rows = page["data"]["audioQueueItems"]
        .as_array()
        .unwrap_or_else(|| panic!("{page}"));
    assert_eq!(rows.len(), 2, "{page}");
    assert_eq!(rows[0]["durationMs"], 1000);

    let count = query(&schema, r#"query { audioQueueItemCount }"#).await;
    assert_eq!(count["data"]["audioQueueItemCount"], 3);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn queue_ids_use_the_existing_items_projection_without_platform_calls() {
    let (_directory, schema) = fixture_with(GRANTED, |method, _| {
        panic!("Unexpected host call: {method}")
    });
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "Alpha", "One"),
            ("/music/b.mp3", "Beta", "Two"),
            ("/music/c.mp3", "Gamma", "Three"),
        ],
    );
    let result = query(
        &schema,
        r#"{
        all: audioQueueItems(query: "", offset: 0, limit: 2147483647) { mediaId }
        page: audioQueueItems(query: "", offset: 1, limit: 1) { mediaId }
        selected: audioQueueItems(query: "ids:test:/music/b.mp3", offset: 0, limit: 10) { mediaId }
    }"#,
    )
    .await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(
        result["data"]["all"],
        json!([
            {"mediaId":"test:/music/a.mp3"}, {"mediaId":"test:/music/b.mp3"}, {"mediaId":"test:/music/c.mp3"}
        ])
    );
    assert_eq!(
        result["data"]["page"],
        json!([{"mediaId":"test:/music/b.mp3"}])
    );
    assert_eq!(result["data"]["selected"], result["data"]["page"]);
}

/// `audioPlayback` merges the store and the player, so both halves are
/// asserted: the queue's current path and the platform's own progress.
#[tokio::test]
async fn playback_joins_the_queue_state_and_the_player_state() {
    let (_dir, schema) = fixture_with(GRANTED, |method, params| match method {
        "syntheticAudioTracks" => json!(
            params["paths"]
                .as_array()
                .unwrap()
                .iter()
                .map(|path| json!({
                    "title": "Track", "artist": "Artist",
                    "path": path.as_str().unwrap(), "durationMs": 1000,
                }))
                .collect::<Vec<_>>()
        ),
        "audioEngineCommand" => engine_report(&params),
        "systemAudioPlaybackState" => json!({ "isPlaying": true, "positionMs": 4200 }),
        _other => panic!("unexpected host call {_other}"),
    });
    seed_queue(&schema, &[("/music/one.mp3", "Track", "Artist")]);
    schema
        .data::<Arc<Prefs>>()
        .unwrap()
        .set("audio_play_mode", "SHUFFLE")
        .unwrap();
    let played = local_query(
        &schema,
        r#"mutation { audioPlayPath(path: "/music/one.mp3") { path } }"#,
    )
    .await;
    assert!(played.get("errors").is_none(), "{played}");

    let state = local_query(
        &schema,
        r#"query { audioPlayback { path mode isPlaying positionMs } }"#,
    )
    .await;
    let playback = &state["data"]["audioPlayback"];
    assert_eq!(playback["path"], "/music/one.mp3", "{state}");
    assert_eq!(playback["mode"], "SHUFFLE");
    assert_eq!(playback["isPlaying"], true);
    assert_eq!(playback["positionMs"], 4200);
}

/// Playing a track must mark it current without touching the queue (2026-09-27,
/// T58). `playAudio` used to append the track to the manual queue, and enqueue
/// moves an already-queued track to the tail — so every desktop play jumped the
/// clicked track to the end. Queueing is the caller's job: they combine
/// `addAudioToQueue`/`enqueue` with the play.
#[tokio::test]
async fn playing_a_track_never_changes_the_queue() {
    let (_dir, schema) = fixture_with(GRANTED, |method, params| match method {
        "syntheticAudioTracks" => json!([{
            "title": "Track", "artist": "Artist",
            "path": params["paths"][0].as_str().unwrap(), "durationMs": 1000,
        }]),
        "audioEngineCommand" => engine_report(&params),
        _other => json!({}),
    });
    database(&schema);
    seed_queue(
        &schema,
        &[
            ("/music/one.mp3", "One", "A"),
            ("/music/two.mp3", "Two", "B"),
        ],
    );

    let played = query(
        &schema,
        r#"mutation { audioPlayPath(path: "/music/one.mp3") { path } }"#,
    )
    .await;
    assert_eq!(
        played["data"]["audioPlayPath"]["path"], "/music/one.mp3",
        "{played}"
    );

    let after = query(
        &schema,
        r#"query { audioQueueItems(offset: 0, limit: 10, query: "") { path } }"#,
    )
    .await;
    let paths: Vec<&str> = after["data"]["audioQueueItems"]
        .as_array()
        .unwrap_or_else(|| panic!("{after}"))
        .iter()
        .map(|item| item["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["/music/one.mp3", "/music/two.mp3"],
        "playAudio must leave the queue exactly as it was — got {paths:?}",
    );
}

/// The store keeps "no current track" as an empty string. The contract says
/// null, and an empty path is not a track a client can act on.
#[tokio::test]
async fn an_idle_player_reports_a_null_current_path() {
    let (_dir, schema) = fixture();
    let state = query(
        &schema,
        r#"query { audioPlayback { path mode isPlaying positionMs } }"#,
    )
    .await;
    assert!(state["errors"].is_null(), "{state}");
    assert!(state["data"]["audioPlayback"]["path"].is_null(), "{state}");
    assert_eq!(state["data"]["audioPlayback"]["mode"], "REPEAT");
}

#[tokio::test]
async fn path_lyrics_reject_missing_storage_permission_before_reading_a_sidecar() {
    let (directory, schema) =
        fixture_with("[]", |method, _| panic!("unexpected host call {method}"));
    let track = directory.path().join("private.mp3");
    std::fs::write(directory.path().join("private.lrc"), "owned sidecar lyrics").unwrap();
    let response = query(
        &schema,
        &format!(
            "query {{ audioLyricsByPath(path: {}) }}",
            serde_json::to_string(&track.to_string_lossy()).unwrap()
        ),
    )
    .await;
    assert_eq!(response["errors"][0]["message"], "no_permission");
    assert!(!response.to_string().contains("owned sidecar lyrics"));
}

#[tokio::test]
async fn lyrics_use_embedded_tags_and_missing_tags_are_null() {
    let (dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let track = dir.path().join("a.mp3");
    std::fs::write(&track, []).unwrap();
    let track = track.to_string_lossy().into_owned();
    seed_queue(&schema, &[(&track, "One", "Artist")]);
    let id = format!("test:{track}");
    let none = query(
        &schema,
        &format!(r#"query {{ audioLyrics(mediaId: "{id}") }}"#),
    )
    .await;
    assert!(none["errors"].is_null(), "{none}");
    assert!(none["data"]["audioLyrics"].is_null(), "{none}");

    let (dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let track = dir.path().join("b.mp3");
    let body = b"\x03eng\x00la la la";
    let mut frame = b"USLT".to_vec();
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(body);
    let mut audio = b"ID3\x03\x00\x00".to_vec();
    audio.extend_from_slice(&[0, 0, 0, frame.len() as u8]);
    audio.extend_from_slice(&frame);
    std::fs::write(&track, audio).unwrap();
    std::fs::write(dir.path().join("b.lrc"), "different sidecar").unwrap();
    let track = track.to_string_lossy().into_owned();
    seed_queue(&schema, &[(&track, "Two", "Artist")]);
    let id = format!("test:{track}");
    let some = query(
        &schema,
        &format!(r#"query {{ audioLyrics(mediaId: "{id}") }}"#),
    )
    .await;
    assert_eq!(some["data"]["audioLyrics"], "la la la", "{some}");
    let sidecar = query(
        &schema,
        &format!(
            "query {{ audioLyricsByPath(path: {}) }}",
            serde_json::to_string(&track).unwrap()
        ),
    )
    .await;
    assert!(sidecar["errors"].is_null(), "{sidecar}");
    assert_eq!(sidecar["data"]["audioLyricsByPath"], "different sidecar");
}

#[tokio::test]
async fn clearing_empties_the_queue_and_tells_the_player() {
    let (_dir, schema) = fixture();
    seed_queue(&schema, &[("/music/one.mp3", "One", "Artist")]);
    seed_queue(&schema, &[("/music/a.mp3", "Alpha", "One")]);
    let mut events = schema
        .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()
        .unwrap()
        .subscribe();
    let cleared = query(&schema, r#"mutation { clearAudioQueue }"#).await;
    assert_eq!(cleared["data"]["clearAudioQueue"], true, "{cleared}");

    let count = query(&schema, r#"query { audioQueueItemCount }"#).await;
    assert_eq!(count["data"]["audioQueueItemCount"], 0, "{count}");
    assert_eq!(events.try_recv().unwrap().event_type, "CONTENT_CHANGED");
}

#[tokio::test]
async fn reordering_writes_the_order_the_client_sent() {
    let (_dir, schema) = fixture();
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "Alpha", "One"),
            ("/music/b.mp3", "Beta", "Two"),
            ("/music/c.mp3", "Gamma", "Three"),
        ],
    );
    let ok = query(
        &schema,
        r#"mutation { reorderAudioQueue(mediaIds: ["test:/music/c.mp3", "test:/music/a.mp3", "test:/music/b.mp3"]) }"#,
    )
    .await;
    assert_eq!(ok["data"]["reorderAudioQueue"], true);

    let items = query(
        &schema,
        r#"query { audioQueueItems(offset: 0, limit: 10, query: "") { path } }"#,
    )
    .await;
    let paths: Vec<&str> = items["data"]["audioQueueItems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["/music/c.mp3", "/music/a.mp3", "/music/b.mp3"],
        "{items}"
    );
}

#[tokio::test]
async fn removing_one_item_leaves_the_rest_of_the_queue() {
    let (_dir, schema) = fixture();
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "Alpha", "One"),
            ("/music/b.mp3", "Beta", "Two"),
        ],
    );
    let ok = query(
        &schema,
        r#"mutation { removeAudioFromQueue(mediaId: "test:/music/a.mp3") }"#,
    )
    .await;
    assert_eq!(ok["data"]["removeAudioFromQueue"], true);

    let items = query(
        &schema,
        r#"query { audioQueueItems(offset: 0, limit: 10, query: "") { path } }"#,
    )
    .await;
    let paths: Vec<&str> = items["data"]["audioQueueItems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["/music/b.mp3"], "{items}");
}

#[tokio::test]
async fn a_playlist_is_created_renamed_and_empties_into_a_count() {
    let (_dir, schema) = fixture();
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "Track", "Artist"),
            ("/music/b.mp3", "Track", "Artist"),
        ],
    );
    let db = database(&schema);
    db.with_conn(|c|c.execute("INSERT INTO audio_playlists(id,name,created_at,updated_at) VALUES('seed','Seed','2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z')",[])).unwrap();
    let tracks = domain::queue_page(db, 0, 10, "").unwrap();
    domain::add_playlist_items(db, "seed", &tracks).unwrap();

    let created = query(
        &schema,
        r#"mutation { createAudioPlaylist(name: "Focus") { id name itemCount } }"#,
    )
    .await;
    let playlist = &created["data"]["createAudioPlaylist"];
    let id = playlist["id"].as_str().unwrap().to_string();
    assert_eq!(playlist["name"], "Focus");
    assert_eq!(playlist["itemCount"], 0);

    let added = query(
        &schema,
        &format!(r#"mutation {{ addAudioPlaylistItems(id: "{id}", query: "playlist_id:seed") }}"#),
    )
    .await;
    assert_eq!(added["data"]["addAudioPlaylistItems"], true, "{added}");

    let count = query(
        &schema,
        &format!(r#"query {{ audioPlaylistItemCount(id: "{id}", query: "") }}"#),
    )
    .await;
    assert_eq!(count["data"]["audioPlaylistItemCount"], 2, "{count}");

    let renamed = query(
        &schema,
        &format!(r#"mutation {{ updateAudioPlaylist(id: "{id}", name: "Deep focus") {{ name itemCount }} }}"#),
    )
    .await;
    assert_eq!(renamed["data"]["updateAudioPlaylist"]["name"], "Deep focus");
    // The update reports the row it changed, count included, so a client
    // never has to re-read the playlist to learn its size.
    assert_eq!(renamed["data"]["updateAudioPlaylist"]["itemCount"], 2);

    let listed = query(&schema, r#"query { audioPlaylists { id name itemCount } }"#).await;
    assert_eq!(
        listed["data"]["audioPlaylists"].as_array().map(Vec::len),
        Some(2)
    );

    let _ = query(
        &schema,
        &format!(r#"mutation {{ deleteAudioPlaylist(id: "{id}") }}"#),
    )
    .await;
    let after = query(&schema, r#"query { audioPlaylists { id } }"#).await;
    assert_eq!(
        after["data"]["audioPlaylists"].as_array().map(Vec::len),
        Some(1)
    );
}

#[tokio::test]
async fn removing_a_playlist_item_lowers_its_count() {
    let (_dir, schema) = fixture();
    seed_queue(&schema, &[("/music/a.mp3", "Track", "Artist")]);
    let db = database(&schema);
    db.with_conn(|c|c.execute("INSERT INTO audio_playlists(id,name,created_at,updated_at) VALUES('seed','Seed','2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z')",[])).unwrap();
    let tracks = domain::queue_page(db, 0, 10, "").unwrap();
    domain::add_playlist_items(db, "seed", &tracks).unwrap();

    let created = query(
        &schema,
        r#"mutation { createAudioPlaylist(name: "Mix") { id } }"#,
    )
    .await;
    let id = created["data"]["createAudioPlaylist"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = query(
        &schema,
        &format!(r#"mutation {{ addAudioPlaylistItems(id: "{id}", query: "playlist_id:seed") }}"#),
    )
    .await;

    let removed = query(
        &schema,
        &format!(r#"mutation {{ removeAudioPlaylistItem(id: "{id}", query: "ids:test:/music/a.mp3") {{ affectedCount }} }}"#),
    )
    .await;
    assert_eq!(
        removed["data"]["removeAudioPlaylistItem"]["affectedCount"], 1,
        "{removed}"
    );

    let count = query(
        &schema,
        &format!(r#"query {{ audioPlaylistItemCount(id: "{id}", query: "") }}"#),
    )
    .await;
    assert_eq!(count["data"]["audioPlaylistItemCount"], 0, "{count}");
}

/// An empty playlist has nothing to start, and the contract types the result
/// nullable for exactly that. An error would make "nothing to play" look
/// like a failure.

#[tokio::test]
async fn play_mode_is_written_and_read_from_rust_preferences() {
    let (_dir, schema) = fixture();
    let ok = query(
        &schema,
        r#"mutation { updateAudioPlayMode(mode: REPEAT_ONE) }"#,
    )
    .await;
    assert_eq!(ok["data"]["updateAudioPlayMode"], true, "{ok}");

    let state = query(&schema, r#"query { audioPlayback { mode } }"#).await;
    assert_eq!(state["data"]["audioPlayback"]["mode"], "REPEAT_ONE");
}

/// Without storage the library degrades rather than erroring — the same rule
/// the image and video lists use — while an explicit browse still refuses.
#[tokio::test]
async fn an_ungranted_library_counts_zero_and_refuses_to_list() {
    let (_dir, schema) = fixture_with("[]", |method, _| match method {
        "systemMediaRows" => json!([{
            "id": "1", "title": "Hidden", "artist": "A", "path": "/m/a.mp3",
            "size": 1, "bucketId": "b", "durationMs": 1, "albumFileId": "",
            "createdAt": "2026-01-01T00:00:00.000Z", "updatedAt": "2026-01-01T00:00:00.000Z",
            "isFavorite": false,
        }]),
        _other => panic!("unexpected host call {_other}"),
    });

    let count = query(&schema, r#"query { audioCount(query: "") }"#).await;
    assert_eq!(count["data"]["audioCount"], 0, "{count}");

    let listed = query(
        &schema,
        r#"query { audios(offset: 0, limit: 10, query: "", sortBy: NAME_ASC) { title } }"#,
    )
    .await;
    assert!(
        !listed["errors"].as_array().is_none_or(Vec::is_empty),
        "{listed}"
    );
}

#[tokio::test]
async fn a_library_row_keeps_the_album_art_reference() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _params| match method {
        _other => panic!("unexpected host call {_other}"),
    });
    database(&schema)
        .record_media_files(&[plain_rs::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type: plain_rs::enums::DataType::Audio,
            path: "/m/a.mp3".to_owned(),
            title: "Song".to_owned(),
            artist: "Band".to_owned(),
            bucket_id: "bucket-1".to_owned(),
            size_bytes: 42,
            duration_ms: 1234,
            cover_image_id: Some("album-file-1".to_owned()),
            width: 0,
            height: 0,
            rotation: 0,
            taken_at: None,
            created_at: "2026-01-01T00:00:00.000Z".to_owned(),
            updated_at: "2026-01-01T00:00:01.000Z".to_owned(),
        }])
        .unwrap();

    let listed = query(
        &schema,
        r#"query { audios(offset: 0, limit: 10, query: "", sortBy: NAME_ASC) {
            id title artist path durationMs size bucketId albumFileId createdAt updatedAt isFavorite tags { name }
        } }"#,
    )
    .await;
    let row = &listed["data"]["audios"][0];
    assert_eq!(row["albumFileId"], "album-file-1", "{listed}");
    assert_eq!(row["artist"], "Band");
    assert_eq!(row["durationMs"], 1234);
    assert_eq!(row["size"], 42);
    assert_eq!(row["bucketId"], "/m");
    assert_eq!(row["tags"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn playlist_items_expose_catalog_identity_and_path() {
    let (_dir, schema) = fixture();
    let db = database(&schema);
    seed_queue(
        &schema,
        &[
            ("/music/first.mp3", "First", "Artist"),
            ("/music/second.mp3", "Second", "Artist"),
        ],
    );
    let playlist = domain::create_playlist(db, "Original contract").unwrap();
    domain::add_playlist_items(
        db,
        &playlist.id,
        &[
            track("/music/first.mp3", "First", "Artist"),
            track("/music/second.mp3", "Second", "Artist"),
        ],
    )
    .unwrap();
    let response = schema.execute(format!(r#"
        query {{ audioPlaylistItems(id: "{}", query: "", offset: 1, limit: 1) {{ ...OriginalTrack }} }}
        fragment OriginalTrack on AudioItem {{ __typename title artist path durationMs }}
    "#, playlist.id)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["audioPlaylistItems"],
        json!([
            {"__typename":"AudioItem", "title":"Second", "artist":"Artist", "path":"/music/second.mp3", "durationMs":1000}
        ])
    );
}

#[tokio::test]
async fn adding_tracks_reads_every_search_page_without_an_item_limit() {
    let (_dir, schema) = fixture_with(GRANTED, |method, params| match method {
        "syntheticAudioTracks" => json!(
            params["paths"]
                .as_array()
                .unwrap()
                .iter()
                .map(|path| json!({"path":path,"title":"Track","artist":"Artist","durationMs":123}))
                .collect::<Vec<_>>()
        ),
        other => panic!("unexpected host call {other}"),
    });
    let rows: Vec<_> = (0..1205)
        .map(|i| plain_rs::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type: DataType::Audio,
            path: format!("/track/{i}"),
            title: format!("Track {i}"),
            artist: "Artist".to_owned(),
            bucket_id: "b".to_owned(),
            size_bytes: 10,
            duration_ms: 123,
            cover_image_id: Some(String::new()),
            width: 0,
            height: 0,
            rotation: 0,
            taken_at: None,
            created_at: "2026-01-01T00:00:00.000Z".to_owned(),
            updated_at: "2026-01-01T00:00:00.000Z".to_owned(),
        })
        .collect();
    database(&schema).record_media_files(&rows).unwrap();

    let added = query(
        &schema,
        r#"mutation { addAudioToQueue(query:"trash:false") }"#,
    )
    .await;
    assert!(added["errors"].is_null(), "{added}");
    let page=query(&schema,r#"{ audioQueueItemCount audioQueueItems(query:"",offset:1200,limit:200) { mediaId path } }"#).await;
    assert_eq!(page["data"]["audioQueueItemCount"], 1205, "{page}");
    let tail = page["data"]["audioQueueItems"].as_array().unwrap();
    assert_eq!(tail.len(), 5, "{page}");
    let paths = tail
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(paths.len(), 5, "{page}");
}

#[tokio::test]
async fn playing_by_id_leaves_order_and_removing_current_clears_the_engine() {
    let clears = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = clears.clone();
    let (_dir, schema) = fixture_with(GRANTED, move |method, params| match method {
        "audioEngineCommand" => {
            assert_eq!(params["action"], "CLEAR");
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            engine_report(&params)
        }
        "systemAudioPlaybackState" => json!({"isPlaying":false,"positionMs":0}),
        method => panic!("unexpected {method}"),
    });
    seed_queue(
        &schema,
        &[
            ("/one", "One", "Artist"),
            ("/two", "Two", "Artist"),
            ("/three", "Three", "Artist"),
        ],
    );
    let played = local_query(
        &schema,
        r#"mutation { playAudio(mediaId:"test:/two") { mediaId path } }"#,
    )
    .await;
    assert_eq!(
        played["data"]["playAudio"]["mediaId"], "test:/two",
        "{played}"
    );
    local_query(
        &schema,
        r#"mutation { reorderAudioQueue(mediaIds:["test:/two","test:/one"]) }"#,
    )
    .await;
    let page = local_query(
        &schema,
        r#"{ audioQueueItems(query:"",offset:0,limit:10) { mediaId } }"#,
    )
    .await;
    assert_eq!(
        page["data"]["audioQueueItems"],
        json!([{"mediaId":"test:/two"},{"mediaId":"test:/one"},{"mediaId":"test:/three"}])
    );
    let removed = local_query(
        &schema,
        r#"mutation { removeAudioFromQueue(mediaId:"test:/two") }"#,
    )
    .await;
    assert!(removed["errors"].is_null(), "{removed}");
    let state = local_query(&schema, "{ audioPlayback { mediaId path } }").await;
    assert!(state["errors"].is_null(), "{state}");
    assert_eq!(
        state["data"]["audioPlayback"]["mediaId"],
        Value::Null,
        "{state}"
    );
    assert_eq!(state["data"]["audioPlayback"]["path"], Value::Null);
    assert_eq!(clears.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn playlist_query_removal_reports_actual_count_without_changing_other_lists() {
    let (_dir, schema) = fixture();
    seed_queue(
        &schema,
        &[("/one", "One", "Artist"), ("/two", "Two", "Artist")],
    );
    let db = database(&schema);
    let first = domain::create_playlist(db, "First").unwrap();
    let second = domain::create_playlist(db, "Second").unwrap();
    let tracks = domain::queue_page(db, 0, 10, "").unwrap();
    domain::add_playlist_items(db, &first.id, &tracks).unwrap();
    domain::add_playlist_items(db, &second.id, &tracks).unwrap();
    let page=query(&schema,&format!(r#"{{ audioPlaylistItems(id:"{}",query:"text:Two",offset:0,limit:10) {{ mediaId }} audioPlaylistItemCount(id:"{}",query:"text:Two") }}"#,first.id,first.id)).await;
    assert_eq!(page["data"]["audioPlaylistItemCount"], 1, "{page}");
    assert_eq!(
        page["data"]["audioPlaylistItems"],
        json!([{"mediaId":"test:/two"}])
    );
    let removed=query(&schema,&format!(r#"mutation {{ removeAudioPlaylistItem(id:"{}",query:"ids:test:/one,test:/absent") {{ affectedCount }} }}"#,first.id)).await;
    assert_eq!(
        removed["data"]["removeAudioPlaylistItem"]["affectedCount"], 1,
        "{removed}"
    );
    assert_eq!(domain::playlist_item_count(db, &second.id).unwrap(), 2);
}

struct AlbumArtFixture(Audio);

#[async_graphql::Object]
impl AlbumArtFixture {
    async fn audio(&self) -> Audio {
        self.0.clone()
    }
}

#[tokio::test]
async fn raw_album_art_is_encoded_only_when_selected_with_the_current_url_token() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let uri = "content://media/external/audio/albumart/17";
    let schema = async_graphql::Schema::build(
        AlbumArtFixture(Audio {
            album_id: "17".into(),
            id: "track".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            path: "/song.mp3".into(),
            duration_ms: Long(1000),
            size: Long(42),
            bucket_id: "music".into(),
            album_file_id: String::new(),
            album_art_uri: uri.into(),
            created_at: Instant::default(),
            updated_at: Instant::default(),
            is_favorite: false,
        }),
        async_graphql::EmptyMutation,
        async_graphql::EmptySubscription,
    )
    .data(prefs.clone())
    .finish();
    prefs.remove(plain_rs::prefs::pref_keys::URL_TOKEN).unwrap();
    for document in [
        "{ audio { title } }",
        "{ audio { title albumFileId @skip(if: true) } }",
    ] {
        let response = schema.execute(document).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["audio"]["title"], "Song");
    }
    let missing = schema.execute("{ audio { albumFileId } }").await;
    assert!(!missing.errors.is_empty());
    let document =
        "query Art { audio { ...Cover } } fragment Cover on Audio { cover: albumFileId }";
    let mut previous: Option<[u8; 32]> = None;
    for key in [[3u8; 32], [5u8; 32]] {
        let token = plain_rs::utils::base64::base64_encode(&key);
        prefs
            .set(plain_rs::prefs::pref_keys::URL_TOKEN, &token)
            .unwrap();
        let response = schema.execute(document).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let id = data["audio"]["cover"].as_str().unwrap();
        let bytes = plain_rs::utils::base64::base64_decode(id);
        assert_eq!(
            plain_rs::crypto::xchacha_decrypt_raw(&key, &bytes).unwrap(),
            uri.as_bytes()
        );
        if let Some(previous) = previous {
            assert!(plain_rs::crypto::xchacha_decrypt_raw(&previous, &bytes).is_none());
        }
        previous = Some(key);
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn main_audio_metadata_reads_unindexed_tags_without_changing_playback_or_stores() {
    let (_directory, schema) =
        fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-tagged.mp3"
    );
    let db = database(&schema);
    let preferences = schema.data::<Arc<Prefs>>().unwrap();
    let before = preferences.entries();
    let current = domain::get_audio_current(db).unwrap();
    let count = domain::queue_total(db).unwrap();
    let request = async_graphql::Request::new("query Metadata($path: String!) { audioMetadata(path: $path) { title artist path durationMs } }")
        .variables(async_graphql::Variables::from_json(json!({"path":path})));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let metadata = response.data.into_json().unwrap();
    assert_eq!(metadata["audioMetadata"]["title"], "Hydrate Title");
    assert_eq!(metadata["audioMetadata"]["artist"], "Hydrate Artist");
    assert_eq!(metadata["audioMetadata"]["path"], path);
    assert!(metadata["audioMetadata"]["durationMs"].as_i64().unwrap() > 0);
    assert_eq!(preferences.entries(), before);
    assert_eq!(domain::get_audio_current(db).unwrap(), current);
    assert_eq!(domain::queue_total(db).unwrap(), count);
    assert!(
        db.media_file_by_path(plain_rs::enums::DataType::Audio, path)
            .unwrap()
            .is_none()
    );
    for path in ["", "/missing/audio.mp3"] {
        let response = schema
            .execute(
                async_graphql::Request::new(
                    "query($path: String!) { audioMetadata(path: $path) { title } }",
                )
                .variables(async_graphql::Variables::from_json(json!({"path":path}))),
            )
            .await;
        assert!(!response.errors.is_empty());
    }
}

#[tokio::test]
async fn main_audio_metadata_requires_storage_authorization_before_reading() {
    let (_directory, schema) =
        fixture_with("[]", |method, _| panic!("unexpected host call {method}"));
    let result = schema
        .execute("{ audioMetadata(path: \"/not-accessed.mp3\") { title } }")
        .await;
    assert!(!result.errors.is_empty());
}

#[tokio::test]
async fn main_audio_cover_extracts_original_bytes_and_preserves_nullable_absence_and_permission_errors()
 {
    let (_directory, schema) =
        fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let request = |path: &str| {
        async_graphql::Request::new(
            "mutation Cover($path: String!) { cacheAudioCover(path: $path) }",
        )
        .variables(async_graphql::Variables::from_json(json!({"path":path})))
    };
    let response = schema
        .execute(request(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/audio-cover.mp3"
        )))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let path = data["cacheAudioCover"].as_str().unwrap();
    assert_eq!(
        std::fs::read(path).unwrap(),
        include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    let response = schema
        .execute(request(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/audio-untagged.mp3"
        )))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(response.data.into_json().unwrap()["cacheAudioCover"].is_null());
    let (_directory, denied) =
        fixture_with("[]", |method, _| panic!("unexpected host call {method}"));
    assert!(
        !denied
            .execute(request("/not-read.mp3"))
            .await
            .errors
            .is_empty()
    );
}

#[tokio::test]
async fn playlist_covers_use_the_shared_catalog_without_platform_requests() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let db = database(&schema);
    db.use_platform_audio_catalog().unwrap();
    db.cache_platform_audio(
        &[plain_rs::db::audio_catalog::AudioMedia {
            file_id: Some("track".into()),
            aliases: Vec::new(),
            track: AudioTrack {
                album_id: "album".into(),
                ..track("/music/a.mp3", "A", "Artist")
            },
        }],
        false,
    )
    .unwrap();
    db.with_conn(|conn| conn.execute(
        "INSERT INTO audio_playlist_items(id,playlist_id,file_id,sort_order,added_at) VALUES('item','playlist','track',0,'now')", [],
    )).unwrap();
    let result = query(&schema, r#"query { audioPlaylistCovers(ids: ["playlist", "empty"]) { id albums { albumId trackCount candidatePaths } } }"#).await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(
        result["data"]["audioPlaylistCovers"],
        json!([
            {"id":"playlist","albums":[{"albumId":"album","trackCount":1,"candidatePaths":["/music/a.mp3"]}]},
            {"id":"empty","albums":[]},
        ])
    );
}

#[tokio::test]
async fn browser_selection_advances_in_rust_without_dispatching_the_server_player() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let browser_db = database(&schema).remote_audio();
    schema
        .data::<Arc<crate::services::audio::host::Audio>>()
        .unwrap()
        .remote()
        .run_playback(|db, _| plain_rs::media::audio::playback::snapshot(db))
        .await
        .unwrap();
    seed_queue(
        &schema,
        &[("/one.mp3", "One", "Artist"), ("/two.mp3", "Two", "Artist")],
    );
    domain::save_audio_current(&browser_db, "/one.mp3").unwrap();
    let local_before = plain_rs::media::audio::playback::snapshot(database(&schema)).unwrap();
    let before = plain_rs::media::audio::playback::snapshot(&browser_db).unwrap();
    let document = format!(
        r#"mutation {{ advanceAudioSelection(action:NEXT,expectedPath:"/one.mp3",expectedRevision:{}) {{ track {{ mediaId path }} revision positionMs }} }}"#,
        before.revision
    );
    let result = query(&schema, &document).await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(
        result["data"]["advanceAudioSelection"]["track"]["path"],
        "/two.mp3"
    );
    assert_eq!(result["data"]["advanceAudioSelection"]["positionMs"], 0);
    assert_ne!(
        result["data"]["advanceAudioSelection"]["revision"],
        before.revision
    );
    let duplicate = query(&schema, &document).await;
    assert!(duplicate["errors"].is_null(), "{duplicate}");
    assert!(duplicate["data"]["advanceAudioSelection"].is_null());
    assert_eq!(domain::get_audio_current(&browser_db).unwrap(), "/two.mp3");
    assert_eq!(
        plain_rs::media::audio::playback::snapshot(database(&schema)).unwrap(),
        local_before
    );
    let mode = query(
        &schema,
        "mutation { updateAudioPlayMode(mode: REPEAT_ONE) }",
    )
    .await;
    assert!(mode["errors"].is_null(), "{mode}");
    assert_eq!(mode["data"]["updateAudioPlayMode"], true);
}

#[tokio::test]
async fn playback_snapshot_reuses_identity_and_reads_sdk_playback_facts_once() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let (_dir, schema) = fixture_with(GRANTED, move |method, _| {
        assert_eq!(method, "systemAudioPlaybackState");
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        json!({"isPlaying":true,"positionMs":123})
    });
    schema
        .data::<Arc<crate::services::audio::host::Audio>>()
        .unwrap()
        .run_playback(|db, _| plain_rs::media::audio::playback::snapshot(db))
        .await
        .unwrap();
    seed_queue(&schema, &[("/one.mp3", "One", "Artist")]);
    domain::save_audio_current(database(&schema), "/one.mp3").unwrap();
    let expected = plain_rs::media::audio::playback::snapshot(database(&schema)).unwrap();
    let result = local_query(
        &schema,
        "{ audioPlayback { mediaId path revision isPlaying positionMs } }",
    )
    .await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(result["data"]["audioPlayback"]["path"], "/one.mp3");
    assert_eq!(
        result["data"]["audioPlayback"]["revision"],
        expected.revision
    );
    assert_eq!(result["data"]["audioPlayback"]["positionMs"], 123);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn overview_projects_the_authoritative_catalog_without_sdk_metadata_calls() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let (_dir, schema) = fixture_with(GRANTED, move |method, _| {
        assert_eq!(method, "systemCapabilities");
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        json!(["AUDIO_LIBRARY"])
    });
    let db = database(&schema);
    let rows = db
        .record_media_files(&[
            plain_rs::db::MediaFileMetadata {
                data_type: plain_rs::enums::DataType::Audio,
                path: "/one.mp3".into(),
                title: "One".into(),
                artist: "Artist".into(),
                album: String::new(),
                album_artist: String::new(),
                bucket_id: String::new(),
                size_bytes: 1,
                duration_ms: 0,
                cover_image_id: None,
                width: 0,
                height: 0,
                rotation: 0,
                taken_at: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
            plain_rs::db::MediaFileMetadata {
                data_type: plain_rs::enums::DataType::Audio,
                path: "/two.mp3".into(),
                title: "Two".into(),
                artist: "Artist".into(),
                album: String::new(),
                album_artist: String::new(),
                bucket_id: String::new(),
                size_bytes: 1,
                duration_ms: 0,
                cover_image_id: None,
                width: 0,
                height: 0,
                rotation: 0,
                taken_at: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        ])
        .unwrap();
    db.media_play_history_record(plain_rs::enums::DataType::Audio, &rows[1].id, 0)
        .unwrap();
    let result = query(&schema,"{ audioOverview { artists { name itemCount playCount samplePath } recentItems { id path } } }").await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(
        result["data"]["audioOverview"]["artists"],
        json!([
            {"name":"Artist","itemCount":2,"playCount":1,"samplePath":"/one.mp3"},
        ])
    );
    assert_eq!(
        result["data"]["audioOverview"]["recentItems"],
        json!([{"id":rows[1].id,"path":"/two.mp3"}])
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn overview_requires_an_implemented_audio_library_capability() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| {
        assert_eq!(method, "systemCapabilities");
        json!([])
    });
    let result = query(&schema, "{ audioOverview { artists { name } } }").await;
    assert!(result["errors"].is_array(), "{result}");
    assert!(
        result["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("AUDIO_LIBRARY")
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn native_album_tags_reach_playlist_covers_without_platform_catalog_calls() {
    let (directory, schema) =
        fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata/audio-album-tagged.mp3"),
    )
    .unwrap();
    let first = directory.path().join("one.mp3");
    let second = directory.path().join("two.mp3");
    std::fs::write(&first, &bytes).unwrap();
    let mut guest = bytes;
    let offset = guest
        .windows(b"Track Artist".len())
        .position(|part| part == b"Track Artist")
        .unwrap();
    guest[offset..offset + b"Guest Artist".len()].copy_from_slice(b"Guest Artist");
    std::fs::write(&second, guest).unwrap();
    let metadata = [&first, &second]
        .into_iter()
        .map(|path| {
            plain_media::metadata::file_facts::read_file_metadata(path, DataType::Audio).unwrap()
        })
        .collect::<Vec<_>>();
    let rows = database(&schema).record_media_files(&metadata).unwrap();
    assert_eq!(rows[0].artist, "Track Artist");
    assert_eq!(rows[1].artist, "Guest Artist");
    assert_eq!(rows[0].album_id, rows[1].album_id);
    assert!(!rows[0].album_id.is_empty());
    let tracks = rows
        .iter()
        .map(|row| domain::track_by_id(database(&schema), &row.id).unwrap())
        .collect::<Vec<_>>();
    let playlist = domain::create_playlist(database(&schema), "Album").unwrap();
    domain::add_playlist_items(database(&schema), &playlist.id, &tracks).unwrap();
    let result = query(&schema,&format!(r#"{{ audioPlaylistCovers(ids:["{}"]) {{ albums {{ albumId trackCount candidatePaths }} }} }}"#,playlist.id)).await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(
        result["data"]["audioPlaylistCovers"][0]["albums"],
        json!([{
            "albumId":rows[0].album_id,"trackCount":2,"candidatePaths":[rows[0].path,rows[1].path],
        }])
    );
}

#[tokio::test]
async fn browser_history_writes_do_not_change_the_current_player_or_control_the_platform_player() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    seed_queue(&schema, &[("/one.mp3", "One", "Artist")]);
    let local_before = plain_rs::media::audio::playback::snapshot(database(&schema)).unwrap();
    let db = database(&schema);
    let mut events = schema
        .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()
        .unwrap()
        .subscribe();
    for (method, position) in [
        ("recordMediaPlay", 123),
        ("saveMediaPlayProgress", 234),
        ("saveMediaPlayProgress", 456),
    ] {
        let response = query(&schema, &format!(r#"mutation {{ {method}(mediaId:"test:/one.mp3",type:AUDIO,positionMs:{position}) }}"#)).await;
        assert!(response["errors"].is_null(), "{response}");
        assert_eq!(response["data"][method], true);
        assert!(events.try_recv().is_err());
    }
    let history = play_history::matching(db, &[]).unwrap();
    assert_eq!(history[0].play_count, 1);
    assert_eq!(history[0].position_ms, 456);
    let next = query(
        &schema,
        r#"mutation { recordMediaPlay(mediaId:"test:/one.mp3",type:AUDIO,positionMs:0) }"#,
    )
    .await;
    assert!(next["errors"].is_null(), "{next}");
    assert_eq!(play_history::matching(db, &[]).unwrap()[0].play_count, 2);
    assert_eq!(
        plain_rs::media::audio::playback::snapshot(database(&schema)).unwrap(),
        local_before
    );
}

#[tokio::test]
async fn browser_history_writes_require_storage_authorization() {
    let (_dir, schema) = fixture_with("[]", |method, _| panic!("unexpected host call {method}"));
    let response = query(
        &schema,
        r#"mutation { recordMediaPlay(mediaId:"test:/one.mp3",type:AUDIO,positionMs:0) }"#,
    )
    .await;
    assert!(!response["errors"].is_null(), "{response}");
    assert!(
        play_history::matching(database(&schema), &[])
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn queue_query_id_selection_keeps_order_and_rejects_missing_ids_before_insertion() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    seed_queue(
        &schema,
        &[
            ("/music/a.mp3", "A", "Artist"),
            ("/music/b.mp3", "B", "Artist"),
        ],
    );
    let db = &database(&schema).remote_audio();
    domain::clear_queue(db).unwrap();
    let response = query(&schema, r#"mutation {addAudioToQueue(query:"ids:test:/music/b.mp3,test:/music/a.mp3,test:/music/b.mp3")}"#).await;
    assert_eq!(response["data"]["addAudioToQueue"], true, "{response}");
    let rows = domain::queue_page(db, 0, 10, "").unwrap();
    assert_eq!(
        rows.iter().map(|row| row.path.as_str()).collect::<Vec<_>>(),
        ["/music/b.mp3", "/music/a.mp3"]
    );
    assert_eq!(domain::get_audio_current(db).unwrap(), "/music/b.mp3");
    let selected = plain_rs::media::audio::playback::snapshot(db).unwrap();
    let response = query(
        &schema,
        r#"mutation {addAudioToQueue(query:"ids:test:/music/a.mp3")}"#,
    )
    .await;
    assert_eq!(response["data"]["addAudioToQueue"], true);
    assert_eq!(
        plain_rs::media::audio::playback::snapshot(db).unwrap(),
        selected
    );
    for selection in [
        "ids:test:/music/b.mp3,test:/missing",
        "ids:",
        "ids:test:/music/a.mp3,",
    ] {
        domain::clear_queue(db).unwrap();
        let response = query(
            &schema,
            &format!("mutation {{addAudioToQueue(query:{})}}", json!(selection)),
        )
        .await;
        assert!(response.get("errors").is_some(), "{response}");
        assert!(domain::queue_page(db, 0, 10, "").unwrap().is_empty());
    }
}
