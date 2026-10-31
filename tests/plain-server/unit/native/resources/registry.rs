use super::*;
use std::{future::Future, task::Poll};

#[tokio::test]
async fn resource_ranges_span_bounded_reads_and_preserve_exact_file_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.bin");
    let bytes = (0..MAX_RESOURCE_READ_BYTES * 3 + 17)
        .map(|i| (i % 251) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, &bytes).unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let info = registry.open(&scope, &path).await.unwrap();
    assert_eq!(info.name, "fixture.bin");
    assert_eq!(info.size, bytes.len() as u64);
    let resource = registry.get(&scope, &info.id).unwrap();
    let length = MAX_RESOURCE_READ_BYTES * 2 + 1;
    assert_eq!(
        resource.read_range(7, length as u64).await.unwrap(),
        bytes[7..7 + length]
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[tokio::test]
async fn another_scope_cannot_read_or_close_a_resource_and_owner_close_is_isolated() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.txt");
    std::fs::write(&path, b"document").unwrap();
    let registry = NativeResourceRegistry::default();
    let owner = NativeRequestScope::default();
    let other = NativeRequestScope::default();
    let first = registry.open(&owner, &path).await.unwrap();
    let second = registry.open(&other, &path).await.unwrap();
    assert_ne!(first.id, second.id);
    assert!(matches!(
        registry.get(&other, &first.id),
        Err(ResourceAccessError::Forbidden)
    ));
    assert!(!registry.close(&other, &first.id));
    let resource = registry.get(&owner, &first.id).unwrap();
    assert_eq!(resource.read_range(0, 8).await.unwrap(), b"document");
    registry.close_owner(owner.id());
    assert!(resource.read_range(0, 8).await.is_err());
    assert!(matches!(
        registry.get(&owner, &first.id),
        Err(ResourceAccessError::Unavailable)
    ));
    assert_eq!(
        registry
            .get(&other, &second.id)
            .unwrap()
            .read_range(0, 8)
            .await
            .unwrap(),
        b"document"
    );
    let reopened = registry.open(&owner, &path).await.unwrap();
    assert_ne!(reopened.id, first.id);
    assert!(resource.read_range(0, 8).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"document");
}

#[tokio::test]
async fn empty_ranges_limits_and_closed_handles_are_checked_before_reading() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("empty.txt");
    std::fs::write(&path, b"").unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let info = registry.open(&scope, &path).await.unwrap();
    let resource = registry.get(&scope, &info.id).unwrap();
    assert!(resource.read_range(0, 0).await.unwrap().is_empty());
    assert!(resource.read_range(1, 0).await.is_err());
    assert_eq!(
        resource
            .read_range(0, MAX_RESOURCE_RESPONSE_BYTES + 1)
            .await
            .unwrap_err(),
        "resource response exceeds limit"
    );
    assert!(resource.read_range(u64::MAX, 1).await.is_err());
    assert!(registry.close(&scope, &info.id));
    assert!(resource.read_range(0, 0).await.is_err());
    assert!(!registry.close(&scope, &info.id));
}

#[tokio::test]
async fn closing_the_scope_interrupts_a_read_waiting_for_the_file_lock() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.txt");
    std::fs::write(&path, b"document").unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let info = registry.open(&scope, &path).await.unwrap();
    let resource = registry.get(&scope, &info.id).unwrap();
    let slot = resource
        .store
        .state
        .lock()
        .unwrap()
        .resources
        .get(&info.id)
        .unwrap()
        .clone();
    let guard = slot.file.lock().await;
    let mut read = Box::pin(resource.read_range(0, 8));
    assert!(std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx).is_pending())).await);
    scope.close();
    registry.close_owner(scope.id());
    assert!(read.await.is_err());
    drop(guard);
    assert!(registry.open(&scope, &path).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"document");
}

#[tokio::test]
async fn registry_drop_invalidates_held_resources_without_deleting_the_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.txt");
    std::fs::write(&path, b"document").unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let info = registry.open(&scope, &path).await.unwrap();
    let resource = registry.get(&scope, &info.id).unwrap();
    drop(registry);
    assert!(resource.read_range(0, 8).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"document");
}

#[tokio::test]
async fn absent_files_and_directories_do_not_register_resource_handles() {
    let root = tempfile::tempdir().unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    assert!(registry.open(&scope, root.path()).await.is_err());
    assert!(
        registry
            .open(&scope, &root.path().join("missing"))
            .await
            .is_err()
    );
    assert!(registry.state.lock().unwrap().handles.is_empty());
}

#[test]
fn a_scope_closed_during_file_open_cannot_register_a_late_handle() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.txt");
    std::fs::write(&path, b"document").unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let registry = NativeResourceRegistry::default();
        let scope = NativeRequestScope::default();
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = blocked.recv();
        });
        ready.await.unwrap();
        let mut opening = Box::pin(registry.open(&scope, &path));
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(opening.as_mut().poll(cx).is_pending())).await
        );
        scope.close();
        registry.close_owner(scope.id());
        let outcome = std::future::poll_fn(|cx| Poll::Ready(opening.as_mut().poll(cx))).await;
        assert!(matches!(outcome, Poll::Ready(Err(_))));
        drop(opening);
        release.send(()).unwrap();
        blocker.await.unwrap();
        let fresh = NativeRequestScope::default();
        let info = registry.open(&fresh, &path).await.unwrap();
        assert_eq!(registry.state.lock().unwrap().handles.len(), 1);
        assert_eq!(
            registry
                .get(&fresh, &info.id)
                .unwrap()
                .read_range(0, 8)
                .await
                .unwrap(),
            b"document"
        );
    });
    drop(runtime);
    assert_eq!(std::fs::read(path).unwrap(), b"document");
}
