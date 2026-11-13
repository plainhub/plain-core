use super::*;
use std::sync::Arc;

#[tokio::test]
async fn download_directories_use_startup_facts_and_preserve_explicit_native_paths() {
    let root = crate::test_runtime::public_directory();
    for default in [root.path().to_str().unwrap(), "", "relative", "bad\0path"] {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let prefs = Arc::new(
            crate::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
        );
        prefs.set("client_id", "fixture").unwrap();
        let mut config = crate::infrastructure::platform::startup::config();
        config.downloads_dir = default.into();
        let core = crate::test_runtime::Core::start_core_with_config(
            directory.path(),
            &plain_libs::crypto::gen_token(),
            prefs,
            config,
        )
        .unwrap();
        let state = core.runtime_state();
        let (generation, mut requests) = state.host.connect();
        let selected = root.path().join("selected");
        assert_eq!(
            target_directory(&state, selected.to_str().unwrap()).unwrap(),
            selected.to_str().unwrap()
        );
        assert!(target_directory(&state, "relative").is_err());
        assert!(target_directory(&state, "bad\0path").is_err());
        if default == root.path().to_str().unwrap() {
            assert_eq!(
                target_directory(&state, "").unwrap(),
                root.path().join("PlainApp").to_str().unwrap()
            );
        } else {
            assert!(target_directory(&state, "").is_err());
        }
        assert!(!selected.exists());
        assert!(requests.try_recv().is_err());
        state.host.disconnect(generation);
        core.shutdown().await;
    }
}
