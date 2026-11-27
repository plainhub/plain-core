use super::*;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::files::resource_link::{self, Resource, Scope};
use plain_rs::features::prefs::pref_keys::API_PERMISSIONS;
use plain_rs::features::sessions::SessionType;
use plain_rs::infrastructure::data_paths::DataPaths;
use std::path::Path;

#[tokio::test]
async fn app_data_preview_follows_file_visibility_without_extending_download_access() {
    let (_root, core, state, _) = test_state();
    let paths = DataPaths::new(&state.directory);
    let log = paths.log_file();
    let fixture = b"synthetic application log\n";
    std::fs::create_dir_all(paths.logs()).unwrap();
    std::fs::write(&log, fixture).unwrap();
    for name in ["prefs-v0.1.backup.json", "local_chat.db", "tls.pem"] {
        std::fs::write(state.directory.join(name), b"protected fixture").unwrap();
    }
    let mut session =
        plain_rs::features::services::sessions::create(&state, "Fixture".into()).unwrap();
    let scope = Scope::Session(session.client_id.clone());
    let key = plain_rs::features::sessions::token_key(&session.url_token).unwrap();
    let link = |path: &Path, action| {
        let id = resource_link::seal(&scope, &key, &Resource::reference(path.to_str().unwrap()))
            .unwrap();
        format!("id={id}&action={action}")
    };
    let preview = link(&log, "PREVIEW");
    state
        .prefs
        .set(API_PERMISSIONS, vec![AccessFeature::Files])
        .unwrap();
    assert_eq!(
        call_fs(&state, &preview, &[]).await.status(),
        StatusCode::FORBIDDEN
    );

    session.r#type = SessionType::Login;
    state.db.session_save(&session).unwrap();
    state
        .prefs
        .set(API_PERMISSIONS, Vec::<AccessFeature>::new())
        .unwrap();
    assert_eq!(
        call_fs(&state, &preview, &[]).await.status(),
        StatusCode::FORBIDDEN
    );

    state
        .prefs
        .set(API_PERMISSIONS, vec![AccessFeature::Files])
        .unwrap();
    let response = call_fs(&state, &preview, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, fixture);
    assert_eq!(
        call_fs(&state, &link(&log, "DOWNLOAD"), &[]).await.status(),
        StatusCode::FORBIDDEN
    );

    for path in [
        paths.logs(),
        paths.database(),
        paths.prefs(),
        paths.tls_identity(),
        paths.tls_certificate(),
        state.directory.join("tls.key"),
        state.directory.join("prefs-v0.1.backup.json"),
        state.directory.join("local_chat.db"),
    ] {
        assert_eq!(
            call_fs(&state, &link(&path, "PREVIEW"), &[]).await.status(),
            StatusCode::FORBIDDEN,
            "{path:?}"
        );
    }
    #[cfg(unix)]
    {
        let alias = paths.logs().join("settings.log");
        std::os::unix::fs::symlink(paths.prefs(), &alias).unwrap();
        assert_eq!(
            call_fs(&state, &link(&alias, "PREVIEW"), &[])
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let listing = plain_rs::features::services::files::browse::execute(
        &state.file_platform,
        plain_libs::filesystem::browse::Request {
            root: state.directory.to_string_lossy().into_owned(),
            query: "show_hidden:true".into(),
            sort_by: plain_libs::filesystem::SortBy::NameAsc,
            offset: 0,
            limit: None,
            count_only: false,
            count_children: false,
        },
    )
    .await
    .unwrap();
    let items = listing["items"].as_array().unwrap();
    assert!(items.iter().any(|item| item["name"] == "logs"));
    for name in [
        "plain.db",
        "prefs.json",
        "prefs-v0.1.backup.json",
        "local_chat.db",
        "tls.pem",
    ] {
        assert!(!items.iter().any(|item| item["name"] == name), "{name}");
    }
    core.stop_core().await;
}
