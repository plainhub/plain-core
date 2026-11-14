use super::*;
use crate::test_runtime::Core as ContentServer;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn concurrent_imports_store_note_images_as_app_files() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let mut tasks = Vec::new();
    for index in 0_u8..8 {
        let source = directory.path().join(format!("source-{index}"));
        std::fs::write(&source, [index; 32]).unwrap();
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
            let reference = import(&state, source.to_str().unwrap(), "picture.png")
                .await
                .unwrap();
            let hash = plain_libs::utils::hex::bytes_to_hex(&Sha256::digest([index; 32]));
            assert_eq!(reference, format!("fid:{hash}.png"));
            let record = state.db.app_file_get(&hash).unwrap().unwrap();
            assert_eq!(
                record.real_path,
                format!("files/{}/{}/{hash}.png", &hash[..2], &hash[2..4])
            );
            assert_eq!(record.ref_count, 1);
            let path = crate::features::services::files::references::resolve(
                state.db.clone(),
                state.directory.clone(),
                &reference,
            )
            .unwrap();
            assert_eq!(std::fs::read(path).unwrap(), [index; 32]);
            assert_eq!(std::fs::read(source).unwrap(), [index; 32]);
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(state.db.count_app_files(), 8);
    assert!(!directory.path().join("note-images").exists());
    assert_eq!(
        std::fs::read_dir(state.cache.image_cache())
            .unwrap()
            .count(),
        0
    );
    core.stop_core().await;
}

#[tokio::test]
async fn imports_deduplicate_without_using_picked_names_as_paths() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("input");
    std::fs::write(&source, b"bytes").unwrap();
    let reference = import(&state, source.to_str().unwrap(), "../outside.png")
        .await
        .unwrap();
    let hash = plain_libs::utils::hex::bytes_to_hex(&Sha256::digest(b"bytes"));
    assert_eq!(reference, format!("fid:{hash}.png"));
    assert!(!directory.path().join("outside.png").exists());
    assert_eq!(
        import(&state, source.to_str().unwrap(), "C:picture?.png ")
            .await
            .unwrap(),
        reference
    );
    let record = state.db.app_file_get(&hash).unwrap().unwrap();
    assert_eq!(record.ref_count, 2);
    assert_eq!(state.db.count_app_files(), 1);
    for name in ["CON.png", "nul", "LPT1.jpg", "com9.txt", "bad\0name"] {
        assert!(
            import(&state, source.to_str().unwrap(), name)
                .await
                .is_err()
        );
    }
    assert!(
        import(&state, "/missing-input", "picture.png")
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_dir(state.cache.image_cache())
            .unwrap()
            .count(),
        0
    );
    assert_eq!(std::fs::read(&source).unwrap(), b"bytes");
    core.stop_core().await;
    assert!(
        import(&state, source.to_str().unwrap(), "stopped.png")
            .await
            .is_err()
    );
}
