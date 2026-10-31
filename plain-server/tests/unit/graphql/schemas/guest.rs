use super::*;
use crate::server::ContentServer;
#[tokio::test]
async fn shared_file_rows_read_metadata_directly_without_host_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join("Shared");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("note.txt"), b"shared content").unwrap();
    std::fs::write(folder.join("cover.png"), b"image bytes").unwrap();
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = server.runtime_state();
    let shares = Service::new(state.db.clone(), state.prefs.clone());
    let share = shares
        .create(
            "Files".into(),
            vec![folder.to_string_lossy().into_owned()],
            true,
            None,
        )
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(Guest {
            state,
            id: share.id,
        })
        .finish();
    let result = schema
        .execute(
            "{ sharedInfo(virtualPath: \"Shared/\") { entries { name size mimeType hasThumb } } }",
        )
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    let rows = data["sharedInfo"]["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let note = rows.iter().find(|row| row["name"] == "note.txt").unwrap();
    assert_eq!(note["size"], 14);
    assert_eq!(note["mimeType"], "text/plain");
    assert_eq!(note["hasThumb"], false);
    let cover = rows.iter().find(|row| row["name"] == "cover.png").unwrap();
    assert_eq!(cover["size"], 11);
    assert_eq!(cover["mimeType"], "image/png");
    assert_eq!(cover["hasThumb"], true);
    assert!(requests.try_recv().is_err());
    server.stop_core().await;
}

#[test]
fn guest_schema_matches_committed_sdl() {
    let actual = super::schema_builder().finish().sdl();
    if std::env::var_os("UPDATE_GRAPHQL_SCHEMAS").is_some() {
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/guest-api.graphqls"),
            &actual,
        )
        .unwrap();
    } else {
        assert_eq!(
            actual,
            include_str!("../../../../../schema/guest-api.graphqls")
        );
    }
}
