use super::*;
use crate::server::ContentServer;
#[tokio::test]
async fn shared_file_rows_read_metadata_directly_without_host_dispatch() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let folder = directory.path().join("Shared");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("note.txt"), b"shared content").unwrap();
    std::fs::write(folder.join("cover.png"), b"image bytes").unwrap();
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = server.runtime_state();
    let shares = Service::new(state.db.clone(), state.prefs.clone());
    let share = shares
        .create(plain_rs::features::files::shares::Input {
            name: "Files".into(),
            password: String::new(),
            data: (vec![folder.to_string_lossy().into_owned()])
                .into_iter()
                .map(|path: String| {
                    let path = std::path::PathBuf::from(path);
                    plain_rs::features::files::shares::SharePath {
                        virtual_path: format!("/{}", path.file_name().unwrap().to_str().unwrap()),
                        real_path: path.to_str().unwrap().into(),
                        is_dir: path.is_dir(),
                    }
                })
                .collect(),
            read_only: true,
            expires_at: Some("2099-01-01T00:00:00.123Z".into()),
        })
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(Guest {
            state,
            id: share.id.clone(),
        })
        .finish();
    let result = schema
        .execute(
            "{ sharedInfo(relativePath: \"Shared/\") { expiresAt entries { name size mimeType hasThumb } } }",
        )
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["sharedInfo"]["expiresAt"], "2099-01-01T00:00:00.123Z");
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
    shares
        .update(
            &share.id,
            plain_rs::features::files::shares::Input {
                name: "Files".into(),
                password: String::new(),
                read_only: true,
                expires_at: None,
                data: Service::roots(&share).unwrap(),
            },
        )
        .unwrap();
    let response = schema.execute("{ sharedInfo { expiresAt } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(response.data.into_json().unwrap()["sharedInfo"]["expiresAt"].is_null());
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

#[tokio::test]
async fn guest_password_protects_entries_and_file_credentials() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let file = directory.path().join("original.txt");
    std::fs::write(&file, b"original").unwrap();
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let row = service
        .create(plain_rs::features::files::shares::Input {
            name: "Protected".into(),
            password: "secret".into(),
            expires_at: None,
            read_only: true,
            data: vec![plain_rs::features::files::shares::SharePath {
                virtual_path: "/public.txt".into(),
                real_path: file.to_str().unwrap().into(),
                is_dir: false,
            }],
        })
        .unwrap();
    let schema = super::schema(state, row.id.clone());
    for password in [None, Some("wrong"), Some("")] {
        let result = schema.execute(plain_graphql::Request::new("query($password:String){sharedInfo(password:$password){urlToken entries{name}}}")
            .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"password":password})))).await;
        assert!(!result.errors.is_empty());
        assert!(result.data.into_json().unwrap().is_null());
    }
    let result = schema.execute("{sharedInfo(password:\"secret\"){requiresPassword urlToken entries{name relativePath size updatedAt}}}").await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["sharedInfo"]["entries"][0]["name"], "public.txt");
    assert_eq!(data["sharedInfo"]["entries"][0]["size"], 8);
    assert_eq!(data["sharedInfo"]["urlToken"], row.url_token);
    server.shutdown().await;
}
