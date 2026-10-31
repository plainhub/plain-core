use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::db::Db;
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::{fs, sync::Arc};

const GRANTED: &str = r#"["WRITE_EXTERNAL_STORAGE"]"#;

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

fn fixture(permissions: &str) -> (tempfile::TempDir, PublicSchema) {
    fixture_with(permissions, |method, params| match method {
        "systemPermissionFacts" => {
            assert_eq!(params["permissions"], json!(["WRITE_EXTERNAL_STORAGE"]));
            json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}})
        }
        other => panic!("unexpected host call {other}"),
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
            json!(serde_json::from_str::<Vec<String>>(permissions).unwrap()),
        )
        .unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema =
        crate::test_support::schema::build(host.clone(), events, prefs, db, dir.path().into());
    stub(host, handler);
    (dir, schema)
}

fn schema_host(schema: &PublicSchema) -> Arc<Host> {
    schema.data::<Arc<Host>>().unwrap().clone()
}

fn sample_tree(dir: &std::path::Path) -> std::path::PathBuf {
    let root = dir.join("storage");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("alpha.txt"), b"alpha").unwrap();
    fs::write(root.join("beta.png"), b"beta").unwrap();
    fs::write(root.join("nested/gamma.txt"), b"gamma").unwrap();
    root
}

#[tokio::test]
async fn files_are_gated_on_storage_permission() {
    let (_dir, schema) = fixture("[]");
    let response = schema
        .execute(
            r#"query { files(root:"/storage", offset:0, limit:10, query:"", sortBy:NAME_ASC) { name } }"#,
        )
        .await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "no_permission");
}

#[tokio::test]
async fn a_directory_page_is_read_through_the_shared_filesystem_walk() {
    let (dir, schema) = fixture(GRANTED);
    let root = sample_tree(dir.path());
    let response = schema
        .execute(&format!(
            r#"query {{ files(root:{:?}, offset:0, limit:10, query:"", sortBy:NAME_ASC)
                      {{ name path size isDir childCount mediaId }} }}"#,
            root.to_string_lossy()
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let items = data["files"].as_array().unwrap();
    let names: Vec<&str> = items
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    // Directories first, then files by name — the same ordering the app's
    // own file browser sees.
    assert_eq!(names, vec!["nested", "alpha.txt", "beta.png"]);
    assert_eq!(items[0]["isDir"], true);
    assert_eq!(items[1]["isDir"], false);
    assert_eq!(items[1]["size"], 5);
    assert_eq!(items[0]["childCount"], 1);
    // The empty sentinel is a null id, never "".
    assert_eq!(items[1]["mediaId"], Value::Null);
}

#[tokio::test]
async fn a_file_count_ignores_paging_but_honours_the_query() {
    let (dir, schema) = fixture(GRANTED);
    let root = sample_tree(dir.path());
    let response = schema
        .execute(&format!(
            r#"query {{ all: fileCount(root:{:?}, query:"")
                      txt: fileCount(root:{:?}, query:".txt") }}"#,
            root.to_string_lossy(),
            root.to_string_lossy()
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["all"], 3);
    // A bare token is the DSL's name filter, and it searches recursively —
    // so gamma.txt counts even though it is one level down.
    assert_eq!(data["txt"], 2);
}

#[tokio::test]
async fn the_page_and_the_count_describe_the_same_query() {
    let (dir, schema) = fixture(GRANTED);
    let root = sample_tree(dir.path());
    let response = schema
        .execute(&format!(
            r#"query {{ files(root:{:?}, offset:0, limit:2, query:"", sortBy:SIZE_DESC) {{ name }}
                      fileCount(root:{:?}, query:"") }}"#,
            root.to_string_lossy(),
            root.to_string_lossy()
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["files"].as_array().unwrap().len(), 2);
    assert_eq!(data["fileCount"], 3);
}

#[tokio::test]
async fn file_info_combines_rust_gps_with_sdk_decode_bounds() {
    let (dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}),
        "imageDecodeBounds" => json!({"width": 4032, "height": 3024}),
        other => panic!("unexpected host call {other}"),
    });
    let path = dir.path().join("pic.jpg");
    fs::write(&path, image_gps_metadata()).unwrap();
    let response = schema
        .execute(async_graphql::Request::new(format!(
            r#"query {{ fileInfo(path:{:?}) {{ path size updatedAt
                      data {{ ... on ImageFileInfo {{ width height location {{ latitude longitude }} }} }} }} }}"#,
            path.to_string_lossy()
)).data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["fileInfo"]["size"], 200);
    assert_eq!(data["fileInfo"]["data"]["width"], 4032);
    assert_eq!(data["fileInfo"]["data"]["height"], 3024);
    assert_eq!(data["fileInfo"]["data"]["location"]["latitude"], 1.5);
    assert_eq!(data["fileInfo"]["data"]["location"]["longitude"], -2.5);
}

#[tokio::test]
async fn the_display_name_selects_the_media_type_and_rust_sniffs_the_container() {
    let (dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}),
        other => panic!("unexpected host call {other}"),
    });
    let path = dir.path().join("blob");
    fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/audio-untagged.mp3"
        ),
        &path,
    )
    .unwrap();
    let response = schema
        .execute(async_graphql::Request::new(format!(
            r#"query {{ fileInfo(path:{:?}, fileName:"track.m4a")
                      {{ data {{ ... on AudioFileInfo {{ durationMs location {{ latitude longitude }} }} }} }} }}"#,
            path.to_string_lossy()
)).data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert!(data["fileInfo"]["data"]["durationMs"].as_i64().unwrap() > 0);
    assert_eq!(data["fileInfo"]["data"]["location"], Value::Null);
}

/// A plain-text name is not media: no probe runs at all, and `data` is null.
#[tokio::test]
async fn a_non_media_name_probes_nothing() {
    let (dir, schema) = fixture(GRANTED);
    let path = dir.path().join("notes.txt");
    fs::write(&path, b"notes").unwrap();
    let response = schema
        .execute(&format!(
            r#"query {{ fileInfo(path:{:?}) {{ data {{ ... on ImageFileInfo {{ width }} }} }} }}"#,
            path.to_string_lossy()
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["fileInfo"]["data"],
        Value::Null
    );
}

/// A path that cannot be resolved is not an error: the panel renders zeros
/// so a stale link degrades instead of blowing up the whole page.
#[tokio::test]
async fn a_missing_path_reports_zeros_rather_than_failing() {
    let (_dir, schema) = fixture(GRANTED);
    let response = schema
        .execute(r#"query { fileInfo(path:"/nope/missing.jpg") { path size updatedAt data { ... on ImageFileInfo { width } } } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["fileInfo"]["size"], 0);
    assert_eq!(data["fileInfo"]["updatedAt"], "1970-01-01T00:00:00.000Z");
    assert_eq!(data["fileInfo"]["data"], Value::Null);
}

#[tokio::test]
async fn path_predicates_answer_for_files_and_directories() {
    let (dir, schema) = fixture(GRANTED);
    let root = sample_tree(dir.path());
    let file = root.join("alpha.txt");
    let response = schema
        .execute(&format!(
            r#"query {{ f: pathExists(path:{:?}) fk: pathKind(path:{:?})
                      d: pathExists(path:{:?}) dk: pathKind(path:{:?}) }}"#,
            file.to_string_lossy(),
            file.to_string_lossy(),
            root.to_string_lossy(),
            root.to_string_lossy()
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["f"], true);
    assert_eq!(data["fk"], "FILE");
    assert_eq!(data["d"], true);
    assert_eq!(data["dk"], "DIR");
}

/// The total predicate: blank, "." and missing all read false / null and
/// none of them raise, so clients can filter on it unguarded.
#[tokio::test]
async fn unresolvable_paths_are_false_rather_than_errors() {
    let (_dir, schema) = fixture(GRANTED);
    let response = schema
        .execute(
            r#"query { blank: pathExists(path:"") dot: pathExists(path:".")
                      missing: pathExists(path:"/nope")
                      blankKind: pathKind(path:"") missingKind: pathKind(path:"/nope") }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["blank"], false);
    assert_eq!(data["dot"], false);
    assert_eq!(data["missing"], false);
    assert_eq!(data["blankKind"], Value::Null);
    assert_eq!(data["missingKind"], Value::Null);
}

/// A refused root is indistinguishable from a missing one on purpose —
/// `pathExists` must not become a probe for what the client may read.
#[tokio::test]
async fn a_denied_root_reads_as_absent() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":false}}),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"query { exists: pathExists(path:"/data/user/0/secret") kind: pathKind(path:"/data/user/0/secret") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["exists"], false);
    assert_eq!(data["kind"], Value::Null);
}

/// The decoder must not run for a path the authorization refused: the
/// probe reads the file's header, so running it on a denied path would
/// hand a client dimensions and GPS for bytes it may not read. The stub
/// panics on any media call, so this fails loudly if one is made.
#[tokio::test]
async fn a_denied_path_is_never_probed_for_media() {
    let (_dir, schema) = fixture_with(GRANTED, |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":false}}),
        other => panic!("media probe ran for a denied path: {other}"),
    });
    let response = schema
        .execute(
            r#"query { fileInfo(path:"/data/user/0/secret.jpg")
                      { path size data { ... on ImageFileInfo { width } } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["fileInfo"]["size"], 0);
    assert_eq!(data["fileInfo"]["data"], Value::Null);
}

/// These two carry no permission gate: enumerating volumes and reading the
/// bookmarks table is not a storage read of user content.
#[tokio::test]
async fn mounts_and_favorites_need_no_storage_grant() {
    let (_dir, schema) = fixture_with("[]", |method, _| match method {
        "systemMountFacts" => json!([
            {"id":"path:/storage","name":"Internal","path":"/storage","mountPoint":"/storage",
             "fsType":"ext4","totalBytes":100,"usedBytes":40,"freeBytes":60,"remote":false,
             "alias":"","driveType":"INTERNAL_STORAGE","diskId":""},
            {"id":"path:/storage/1234-5678","name":"","path":"/storage/1234-5678",
             "mountPoint":"/storage/1234-5678","fsType":"vfat","totalBytes":10,"usedBytes":4,
             "freeBytes":6,"remote":false,"alias":"USB","driveType":"USB_STORAGE","diskId":"1234"},
        ]),
        other => panic!("unexpected host call {other}"),
    });
    schema
        .data::<Arc<plain_rs::services::files::platform::FilePlatform>>()
        .unwrap()
        .use_sdk_for_test();
    let response = schema
        .execute(
            r#"query { mounts { id name mountPoint fsType totalBytes usedBytes freeBytes
                                remote alias driveType diskId } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let mounts = response.data.into_json().unwrap();
    let mounts = mounts["mounts"].as_array().unwrap();
    assert_eq!(mounts[0]["driveType"], "INTERNAL_STORAGE");
    assert_eq!(mounts[0]["totalBytes"], 100);
    assert_eq!(mounts[1]["driveType"], "USB_STORAGE");
    assert_eq!(mounts[1]["alias"], "USB");
}

/// The bookmarks come straight out of Rust SQLite — the host is never
/// asked, because doing so would re-enter this root once `main_graphql`
/// serves the public schema.
#[tokio::test]
async fn favorite_folders_are_read_from_the_rust_store() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    stub(host.clone(), |method, _| {
        panic!("unexpected host call {method}")
    });
    plain_rs::favorites::add_full_path(&db, "/storage/emulated/0", "/storage/emulated/0/DCIM")
        .unwrap();
    plain_rs::favorites::add_full_path(&db, "/storage/emulated/0", "/storage/emulated/0/Download")
        .unwrap();
    plain_rs::favorites::set_alias(&db, "/storage/emulated/0", "DCIM", "Camera").unwrap();
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema = crate::test_support::schema::build(host, events, prefs, db, dir.path().into());

    let response = schema
        .execute(r#"query { favoriteFolders { rootPath fullPath alias } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let items = response.data.into_json().unwrap();
    let items = items["favoriteFolders"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["fullPath"], "/storage/emulated/0/DCIM");
    assert_eq!(items[0]["alias"], "Camera");
    assert_eq!(items[1]["fullPath"], "/storage/emulated/0/Download");
    assert_eq!(items[1]["alias"], Value::Null);
}

#[tokio::test]
async fn recent_files_are_gated_and_projected_from_the_store() {
    let (dir, schema) = fixture_with(GRANTED, |method, _| panic!("unexpected host call {method}"));
    let first = dir.path().join("a.txt");
    let second = dir.path().join("b.mp4");
    std::fs::write(&first, b"hello world!").unwrap();
    std::fs::write(&second, vec![0u8; 4096]).unwrap();
    let _prefs = schema
        .data::<Arc<plain_rs::prefs::Prefs>>()
        .unwrap()
        .clone();
    plain_rs::recent_files::add(
        schema.data::<Arc<Db>>().unwrap().key_values().as_ref(),
        &first.to_string_lossy(),
    )
    .unwrap();
    plain_rs::recent_files::add(
        schema.data::<Arc<Db>>().unwrap().key_values().as_ref(),
        &second.to_string_lossy(),
    )
    .unwrap();

    let response = schema
        .execute(r#"query { recentFiles { name path mediaId size isDir } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let items = data["recentFiles"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{data}");
    assert_eq!(items[0]["name"], "b.mp4");
    assert_eq!(items[0]["size"], 4096);
    assert_eq!(items[1]["name"], "a.txt");
    assert_eq!(items[1]["size"], 12);
    assert_eq!(items[1]["mediaId"], Value::Null);
    assert_eq!(items[0]["isDir"], false);
}

/// Every mutation returns the whole list, so a client re-renders from the
/// response instead of guessing what happened to rows it did not touch.
#[tokio::test]
async fn favorite_folder_mutations_return_the_whole_list() {
    let (_dir, schema) = fixture("[]");
    let response = schema
        .execute(
            r#"mutation { addFavoriteFolder(rootPath:"/storage/emulated/0",
                                fullPath:"/storage/emulated/0/DCIM") { fullPath alias } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["addFavoriteFolder"],
        json!([{"fullPath": "/storage/emulated/0/DCIM", "alias": null}])
    );

    let response = schema
        .execute(
            r#"mutation { setFavoriteFolderAlias(fullPath:"/storage/emulated/0/DCIM", alias:"Camera")
                          { fullPath alias } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let list = response.data.into_json().unwrap();
    let list = &list["setFavoriteFolderAlias"];
    assert_eq!(list[0]["alias"], "Camera");

    let response = schema
        .execute(r#"mutation { removeFavoriteFolder(fullPath:"/storage/emulated/0/DCIM") { fullPath } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["removeFavoriteFolder"],
        json!([])
    );
}

/// Removing a folder that is already gone is a no-op, not an error: the
/// client cannot tell the two apart and would only show a spurious failure.
#[tokio::test]
async fn removing_an_unknown_favorite_folder_is_a_no_op() {
    let (_dir, schema) = fixture("[]");
    let response = schema
        .execute(r#"mutation { removeFavoriteFolder(fullPath:"/nope") { fullPath } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["removeFavoriteFolder"],
        json!([])
    );
    let response = schema
        .execute(r#"mutation { setFavoriteFolderAlias(fullPath:"/nope", alias:"x") { fullPath } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["setFavoriteFolderAlias"],
        json!([])
    );
}

/// `TAKEN_AT_DESC` has no filesystem equivalent — it is an EXIF-time sort
/// the directory walk cannot do, so it must not be mistaken for a name
/// sort that would silently return a different page.
#[test]
fn sort_by_maps_onto_what_the_directory_walk_can_do() {
    assert!(matches!(
        FileSortBy::DateDesc.into(),
        plain_rs::files::filesystem::SortBy::DateDesc
    ));
    assert!(matches!(
        FileSortBy::SizeAsc.into(),
        plain_rs::files::filesystem::SortBy::SizeAsc
    ));
    assert!(matches!(
        FileSortBy::NameDesc.into(),
        plain_rs::files::filesystem::SortBy::NameDesc
    ));
    assert!(matches!(
        FileSortBy::TakenAtDesc.into(),
        plain_rs::files::filesystem::SortBy::NameAsc
    ));
}

#[tokio::test]
async fn video_details_and_stat_only_aliases_use_rust_without_whole_host_metadata() {
    let (dir, schema) = fixture_with(GRANTED, move |method, _| {
        panic!("Unexpected platform operation {method}")
    });
    let path = dir.path().join("video.mp4");
    fs::write(&path, b"sample").unwrap();
    let execute = |query: &str| {
        async_graphql::Request::new(query)
            .variables(async_graphql::Variables::from_json(json!({"path":path})))
            .data(plain_rs::services::files::info::Info::sdk(schema_host(
                &schema,
            )))
    };
    let response = schema
        .execute(execute(
            "query($path:String!){alias:fileInfo(path:$path){size path}}",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["alias"]["size"], 6);
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/video-h264-baseline.mp4"),
        &path,
    )
    .unwrap();
    let response = schema.execute(execute("query($path:String!){fileInfo(path:$path){metadata:data{... on VideoFileInfo{width height rotation displayWidth displayHeight durationMs bitrate frameRate takenAt title artist location{latitude longitude}}}}}")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let facts = &data["fileInfo"]["metadata"];
    assert_eq!(facts["width"], 320);
    assert_eq!(facts["height"], 240);
    assert_eq!(facts["displayWidth"], 320);
    assert_eq!(facts["displayHeight"], 240);
    assert_eq!(facts["durationMs"], 6000);
    assert!(facts["frameRate"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn native_main_image_info_reads_directly_without_media_host_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let (_generation, mut requests) = core.runtime_state().host.connect();
    let path = directory.path().join("photo.png");
    image::RgbImage::new(3, 2).save(&path).unwrap();
    let response = core.native_graphql().unwrap().execute(async_graphql::Request::new("query($path:String!){fileInfo(path:$path){data{... on ImageFileInfo{width height}}}}")
        .variables(async_graphql::Variables::from_json(json!({"path":path})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["fileInfo"]["data"],
        json!({"width":3,"height":2})
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn batched_file_paths_use_native_rust_data_without_platform_requests() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let paths = vec![
        "app://icons/image.png",
        "app://../outside",
        "fid:invalid",
        "file:///tmp/file%20name.png",
        "app://icons/image.png",
    ];
    let values = serde_json::to_value(
        core.native_management()
            .unwrap()
            .file_paths(paths.into_iter().map(str::to_owned).collect())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        values[0],
        directory
            .path()
            .join("icons/image.png")
            .to_string_lossy()
            .as_ref()
    );
    assert!(values[1].is_null());
    assert!(values[2].is_null());
    assert_eq!(values[3], "/tmp/file name.png");
    assert_eq!(values[0], values[4]);
    assert!(requests.try_recv().is_err());
    drop(state);
    core.shutdown().await;
}

#[tokio::test]
async fn native_file_actions_resolve_registered_attachments_without_graphql() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("document.txt");
    std::fs::write(&source, b"attachment").unwrap();
    let file = plain_rs::chat::attachments::app_file_store::import_file(
        &state.db,
        &state.directory,
        &source,
        "document.txt",
        "text/plain",
    )
    .unwrap();
    let suffix = std::path::Path::new(&file.real_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let paths = serde_json::to_value(
        core.native_management()
            .unwrap()
            .file_paths(vec![
                format!("fid:{suffix}"),
                format!("fid:{}.wrong", file.id),
                "documents/plain.txt".into(),
            ])
            .unwrap(),
    )
    .unwrap();
    let resolved = std::path::PathBuf::from(paths[0].as_str().unwrap());
    assert!(resolved.is_absolute());
    assert_eq!(std::fs::read(resolved).unwrap(), b"attachment");
    assert!(paths[1].is_null());
    assert_eq!(
        std::path::Path::new(paths[2].as_str().unwrap()),
        state.directory.join("documents/plain.txt")
    );
    assert!(requests.try_recv().is_err());
    assert!(plain_rs::chat::attachments::app_file_store::release(
        &state.db,
        &state.directory,
        &file.id,
    ).unwrap());
    assert_eq!(
        core.native_management()
            .unwrap()
            .file_paths(vec![format!("fid:{suffix}")])
            .unwrap(),
        vec![None]
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_main_audio_info_uses_shared_rust_parsing_without_the_media_feature_or_a_host() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let path = directory.path().join("audio.mp3");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata/audio-untagged.mp3"),
        &path,
    )
    .unwrap();
    let (_generation, mut requests) = core.runtime_state().host.connect();
    let response = core.native_graphql().unwrap().execute(async_graphql::Request::new(
        "query($path:String!){fileInfo(path:$path){data{... on AudioFileInfo{durationMs location{latitude longitude}}}}}"
    ).variables(async_graphql::Variables::from_json(json!({"path":path})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let info = response.data.into_json().unwrap()["fileInfo"]["data"].clone();
    assert_eq!(info["durationMs"].as_i64().unwrap() / 1000, 4);
    assert!(info["location"].is_null());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

fn image_gps_metadata() -> Vec<u8> {
    let mut bytes = vec![0; 200];
    bytes[..4].copy_from_slice(b"II\x2a\0");
    bytes[4..8].copy_from_slice(&8u32.to_le_bytes());
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0x8825u16.to_le_bytes());
    bytes[12..14].copy_from_slice(&4u16.to_le_bytes());
    bytes[14..18].copy_from_slice(&1u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..66].copy_from_slice(&4u16.to_le_bytes());
    for (index, tag, kind, count, value) in [
        (0, 1u16, 2u16, 2u32, u32::from(b'N')),
        (1, 2, 5, 3, 128),
        (2, 3, 2, 2, u32::from(b'W')),
        (3, 4, 5, 3, 152),
    ] {
        let start = 66 + index * 12;
        bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[start + 2..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&value.to_le_bytes());
    }
    for (start, degrees) in [(128, 1u32), (152, 2)] {
        for (index, value) in [degrees, 30, 0].into_iter().enumerate() {
            let position = start + index * 8;
            bytes[position..position + 4].copy_from_slice(&value.to_le_bytes());
            bytes[position + 4..position + 8].copy_from_slice(&1u32.to_le_bytes());
        }
    }
    bytes
}

#[tokio::test]
async fn duration_only_media_info_uses_shared_rust_probing_through_aliases_and_fragments() {
    let (directory, schema) = fixture(GRANTED);
    let path = directory.path().join("audio-without-extension");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
        &path,
    )
    .unwrap();
    let document = "query($path:String!){fileInfo(path:$path,fileName:\"audio.mp3\"){metadata:data{__typename ...DurationOnly}}} fragment DurationOnly on AudioFileInfo{time:durationMs}";
    let request = async_graphql::Request::new(document)
        .variables(async_graphql::Variables::from_json(json!({"path":path})))
        .data(plain_rs::services::files::info::Info::sdk(schema_host(
            &schema,
        )));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let metadata = response.data.into_json().unwrap()["fileInfo"]["metadata"].clone();
    assert_eq!(metadata["__typename"], "AudioFileInfo");
    assert_eq!(metadata["time"].as_i64().unwrap() / 1000, 4);
}

#[tokio::test]
async fn type_only_media_info_never_requests_sdk_metadata() {
    let (directory, schema) = fixture(GRANTED);
    let path = directory.path().join("unknown-container");
    fs::write(&path, b"not a media container").unwrap();
    for (name, kind) in [
        ("video.mp4", "VideoFileInfo"),
        ("photo.jpg", "ImageFileInfo"),
        ("audio.mp3", "AudioFileInfo"),
    ] {
        let response = schema.execute(async_graphql::Request::new(
            "query($path:String!,$name:String!){fileInfo(path:$path,fileName:$name){data{__typename}}}"
        ).variables(async_graphql::Variables::from_json(json!({"path":path,"name":name})))
            .data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["fileInfo"]["data"]["__typename"],
            kind
        );
    }
}

#[tokio::test]
async fn duration_only_video_info_does_not_read_stream_or_tag_metadata() {
    let (directory, schema) = fixture(GRANTED);
    let path = directory.path().join("video.mp4");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/video-h264-baseline.mp4"),
        &path,
    )
    .unwrap();
    let response = schema.execute(async_graphql::Request::new(
        "query($path:String!){fileInfo(path:$path){metadata:data{... on VideoFileInfo{time:durationMs}}}}"
    ).variables(async_graphql::Variables::from_json(json!({"path":path})))
        .data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["fileInfo"]["metadata"]["time"]
            .as_i64()
            .unwrap()
            / 1000,
        6
    );
}

#[tokio::test]
async fn unknown_duration_only_file_info_returns_null_without_platform_metadata() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let samples = Arc::new(AtomicUsize::new(0));
    let calls = samples.clone();
    let (directory, schema) = fixture_with(GRANTED, move |method, _| match method {
        "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}),
        other => {
            calls.fetch_add(1, Ordering::SeqCst);
            panic!("Unexpected host operation: {other}");
        }
    });
    let path = directory.path().join("unrecognized.mp4");
    fs::write(&path, b"unsupported container").unwrap();
    let response = schema.execute(async_graphql::Request::new(
        "query($path:String!){fileInfo(path:$path){data{... on VideoFileInfo{durationMs}}}}"
    ).variables(async_graphql::Variables::from_json(json!({"path":path})))
        .data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(response.data.into_json().unwrap()["fileInfo"]["data"].is_null());
    assert_eq!(samples.load(Ordering::SeqCst), 0);
}

#[test]
fn location_projection_preserves_decimal_precision_and_rejects_invalid_points() {
    let point = location(&json!({"rawLocation":"+12.345678-123.456789+3.25/"})).unwrap();
    assert_eq!(point.latitude, 12.345678);
    assert_eq!(point.longitude, -123.456789);
    assert!(location(&json!({"rawLocation":"junk+12.3456-123.1234/"})).is_none());
    assert!(location(&json!({"rawLocation":"+91.0000-123.1234/"})).is_none());
}

#[tokio::test]
async fn file_queries_resolve_shared_references_and_preserve_the_requested_path() {
    let (dir, schema) = fixture(GRANTED);
    let source = dir.path().join("clip.mp4");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/video-h264-baseline.mp4"),
        &source,
    )
    .unwrap();
    let store = plain_rs::files::app_files::FileStore::new(
        schema.data::<Arc<Db>>().unwrap().clone(),
        dir.path().to_path_buf(),
    );
    let stored = store
        .import_file(&source, "clip.mp4", "video/mp4", false)
        .unwrap();
    let suffix = Path::new(&stored.real_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let references = [
        "app://clip.mp4".to_owned(),
        format!("file://{}", source.display()),
        format!("fid:{suffix}"),
    ];
    for reference in references {
        let response = schema.execute(async_graphql::Request::new(
            "query($path:String!){fileInfo(path:$path){path size data{... on VideoFileInfo{width height durationMs}}} pathExists(path:$path) pathKind(path:$path)}"
        ).variables(async_graphql::Variables::from_json(json!({"path":reference})))).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["fileInfo"]["path"], reference);
        assert_eq!(
            data["fileInfo"]["size"],
            fs::metadata(&source).unwrap().len()
        );
        assert_eq!(data["fileInfo"]["data"]["width"], 320);
        assert_eq!(data["fileInfo"]["data"]["height"], 240);
        assert_eq!(data["pathExists"], true);
        assert_eq!(data["pathKind"], "FILE");
        assert_eq!(data["fileInfo"]["data"]["durationMs"], 6000);
    }
    for reference in [
        "",
        " ",
        ".",
        "app://../clip.mp4",
        "fid:invalid",
        "file://remote/clip.mp4",
    ] {
        let response = schema.execute(async_graphql::Request::new(
            "query($path:String!){fileInfo(path:$path){path size data{__typename}} pathExists(path:$path) pathKind(path:$path)}"
        ).variables(async_graphql::Variables::from_json(json!({"path":reference})))).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["fileInfo"]["path"], reference);
        assert_eq!(data["fileInfo"]["size"], 0);
        assert_eq!(data["fileInfo"]["data"], Value::Null);
        assert_eq!(data["pathExists"], false);
        assert_eq!(data["pathKind"], Value::Null);
    }
}

#[tokio::test]
async fn native_mounts_do_not_request_a_host_or_storage_permission() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (_dir, schema) = fixture_with("[]", move |_, _| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        json!([])
    });
    let response = schema
        .execute("{ mounts { id path mountPoint totalBytes freeBytes } }")
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(response.data.into_json().unwrap()["mounts"].is_array());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn mount_projection_preserves_stable_ids_and_explicit_statistics() {
    let partition = mount(&json!({"id":"part:disk:1", "path":null,"mountPoint":null,
        "totalBytes":100,"usedBytes":null,"freeBytes":null}));
    assert_eq!(partition.id.as_str(), "part:disk:1");
    assert!(partition.path.is_empty());
    assert!(partition.mount_point.is_empty());
    let volume = mount(
        &json!({"id":"fsuuid:volume", "path":"/volume", "mountPoint":"/mounted",
        "totalBytes":100,"usedBytes":35,"freeBytes":60}),
    );
    assert_eq!(volume.id.as_str(), "fsuuid:volume");
    assert_eq!(volume.mount_point, "/mounted");
    assert_eq!(volume.used_bytes.0, 35);
    let sdk = mount(&json!({"path":"/storage", "totalBytes":100,"freeBytes":60}));
    assert_eq!(sdk.id.as_str(), "path:/storage");
    assert_eq!(sdk.mount_point, "/storage");
    assert_eq!(sdk.used_bytes.0, 40);
}

#[tokio::test]
async fn audio_duration_selection_probes_unknown_file_names_without_sdk_metadata() {
    let (directory, schema) = fixture(GRANTED);
    for (fixture_name, expected) in [("audio-untagged.mp3", 4), ("video-h264-baseline.mp4", 6)] {
        let path = directory.path().join(fixture_name.replace('.', "-"));
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../plain-rs/testdata")
                .join(fixture_name),
            &path,
        )
        .unwrap();
        let response = schema.execute(async_graphql::Request::new(
            "query($path:String!){fileInfo(path:$path,fileName:\"audio.mp3\"){data{... on AudioFileInfo{durationMs}}}}"
        ).variables(async_graphql::Variables::from_json(json!({"path":path})))
            .data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["fileInfo"]["data"]["durationMs"]
                .as_i64()
                .unwrap()
                / 1000,
            expected
        );
    }
}

#[tokio::test]
async fn audio_duration_selection_keeps_null_for_missing_invalid_and_unknown_sources() {
    let (directory, schema) = fixture(GRANTED);
    let unsupported = directory.path().join("unsupported");
    fs::write(&unsupported, b"unsupported media bytes").unwrap();
    for path in [
        unsupported.to_string_lossy().into_owned(),
        directory
            .path()
            .join("missing")
            .to_string_lossy()
            .into_owned(),
        String::new(),
        "app://../outside".to_owned(),
    ] {
        let response = schema.execute(async_graphql::Request::new(
            "query($path:String!){fileInfo(path:$path,fileName:\"audio.mp3\"){data{... on AudioFileInfo{durationMs}}}}"
        ).variables(async_graphql::Variables::from_json(json!({"path":path})))
            .data(plain_rs::services::files::info::Info::sdk(schema_host(&schema))))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert!(response.data.into_json().unwrap()["fileInfo"]["data"].is_null());
    }
}

#[tokio::test]
async fn audio_duration_selection_preserves_permission_and_probe_errors() {
    let (_directory, schema) = fixture("[]");
    let response = schema.execute(
        "{fileInfo(path:\"missing\",fileName:\"audio.mp3\"){data{... on AudioFileInfo{durationMs}}}}"
    ).await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "no_permission");
    let (directory, schema) = fixture(GRANTED);
    let path = directory.path().join("duration-source.mp3");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
        &path,
    )
    .unwrap();
    schema
        .data::<Arc<plain_rs::services::files::duration_updates::Runtime>>()
        .unwrap()
        .shutdown()
        .await;
    let response = schema.execute(async_graphql::Request::new(
        "query($path:String!){fileInfo(path:$path,fileName:\"audio.mp3\"){data{... on AudioFileInfo{durationMs}}}}"
    ).variables(async_graphql::Variables::from_json(json!({"path":path})))).await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "Media duration runtime stopped");
}
