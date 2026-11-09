use super::*;
use crate::server::ContentServer;

#[tokio::test]
async fn native_references_share_http_path_authorization_without_a_listener() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(root.path()).unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("fixture.bin");
    std::fs::write(&path, b"0123456789").unwrap();
    let resources = core.native_resources().unwrap();
    let info = resources
        .open(&registry, &scope, path.to_str().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(
        registry
            .get(&scope, &info.id)
            .unwrap()
            .read_range(2, 5)
            .await
            .unwrap(),
        b"23456"
    );
    let private = root.path().join("private.txt");
    std::fs::write(&private, b"private fixture").unwrap();
    assert!(
        resources
            .open(&registry, &scope, private.to_str().unwrap(), None)
            .await
            .is_err()
    );
    let opened = registry.get(&scope, &info.id).unwrap();
    core.stop_core().await;
    assert!(opened.read_range(0, 1).await.is_err());
    assert!(registry.get(&scope, &info.id).is_err());
    assert!(
        resources
            .open(&registry, &scope, path.to_str().unwrap(), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn thumbnails_use_shared_content_output_and_not_original_file_bytes() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(root.path()).unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("fixture.png");
    image::RgbImage::from_fn(1600, 1200, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 251) as u8, 97])
    })
    .save(&path)
    .unwrap();
    let resources = core.native_resources().unwrap();
    let original = resources
        .open(&registry, &scope, path.to_str().unwrap(), None)
        .await
        .unwrap();
    let thumbnail = resources
        .open(&registry, &scope, path.to_str().unwrap(), Some(Size::Small))
        .await
        .unwrap();
    assert_ne!(original.id, thumbnail.id);
    assert!(thumbnail.size < original.size);
    assert_eq!(original.mime, "image/png");
    let bytes = registry
        .get(&scope, &thumbnail.id)
        .unwrap()
        .read_range(0, thumbnail.size)
        .await
        .unwrap();
    assert_eq!(thumbnail.mime, content::thumbnail_mime(&bytes));
    let image = image::load_from_memory(&bytes).unwrap();
    assert!(image.width() <= 256);
    assert!(image.height() <= 256);
    let prepared = core.runtime_state().cache.root().join("native_resources");
    assert_eq!(std::fs::read_dir(&prepared).unwrap().count(), 1);
    registry.close_owner(scope.id());
    assert_eq!(std::fs::read_dir(&prepared).unwrap().count(), 0);
    scope.close();
    assert!(registry.get(&scope, &thumbnail.id).is_err());
    core.stop_core().await;
}

#[test]
fn active_document_mime_types_never_receive_inline_native_resources() {
    for mime in [
        "text/html",
        "Text/HTML; charset=utf-8",
        "image/svg+xml",
        " IMAGE/SVG+XML ; charset=UTF-8",
        "application/xhtml+xml",
        "text/xml",
        "application/xml",
    ] {
        assert!(!inline_allowed(mime), "{mime}");
    }
    for mime in [
        "image/png",
        "image/webp",
        "video/mp4",
        "text/plain; charset=utf-8",
        "application/octet-stream",
    ] {
        assert!(inline_allowed(mime), "{mime}");
    }
}

#[tokio::test]
async fn native_export_replaces_only_the_selected_local_destination() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(root.path()).unwrap();
    let files = plain_libs::temporary::tempdir().unwrap();
    let source = files.path().join("source.bin");
    let destination = files.path().join("output.bin");
    std::fs::write(&source, b"original fixture").unwrap();
    std::fs::write(&destination, b"old destination").unwrap();
    let path = core
        .native_resources()
        .unwrap()
        .export(
            &NativeRequestScope::default(),
            None,
            source.to_str().unwrap(),
            ResourceDestination::File(destination.clone()),
        )
        .await
        .unwrap();
    assert_eq!(std::path::Path::new(&path), destination);
    assert_eq!(std::fs::read(&destination).unwrap(), b"original fixture");
    assert_eq!(std::fs::read(&source).unwrap(), b"original fixture");
    assert_eq!(std::fs::read_dir(files.path()).unwrap().count(), 2);
    core.stop_core().await;
}

#[tokio::test]
async fn native_archive_entries_use_the_existing_content_materializer_for_preview_and_export() {
    use std::io::Write;
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(root.path()).unwrap();
    let files = plain_libs::temporary::tempdir().unwrap();
    let archive = files.path().join("fixture.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    zip.start_file("folder/file.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"archive fixture").unwrap();
    zip.finish().unwrap();
    let reference = format!("{}!zip!/folder/file.txt", archive.display());
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let resources = core.native_resources().unwrap();
    let info = resources
        .open(&registry, &scope, &reference, None)
        .await
        .unwrap();
    assert_eq!(info.name, "file.txt");
    assert_eq!(
        registry
            .get(&scope, &info.id)
            .unwrap()
            .read_range(0, info.size)
            .await
            .unwrap(),
        b"archive fixture"
    );
    let destination = files.path().join("export.txt");
    resources
        .export(
            &scope,
            None,
            &reference,
            ResourceDestination::File(destination.clone()),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), b"archive fixture");
    core.stop_core().await;
}

#[tokio::test]
async fn native_export_to_the_same_inode_does_not_replace_its_identity() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(root.path()).unwrap();
    let files = plain_libs::temporary::tempdir().unwrap();
    let source = files.path().join("source.bin");
    let alias = files.path().join("alias.bin");
    std::fs::write(&source, b"original fixture").unwrap();
    std::fs::hard_link(&source, &alias).unwrap();
    core.native_resources()
        .unwrap()
        .export(
            &NativeRequestScope::default(),
            None,
            source.to_str().unwrap(),
            ResourceDestination::File(alias.clone()),
        )
        .await
        .unwrap();
    assert!(plain_libs::utils::file_identity::same(&source, &alias).unwrap());
    assert_eq!(std::fs::read_dir(files.path()).unwrap().count(), 2);
    core.stop_core().await;
}

#[tokio::test]
async fn resource_preparation_is_bounded_before_metadata_or_thumbnail_work() {
    use std::{future::Future, task::Poll};
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(root.path()).unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let mut permits = Vec::new();
    for _ in 0..4 {
        permits.push(registry.prepare().await.unwrap());
    }
    let resources = core.native_resources().unwrap();
    let mut open =
        Box::pin(resources.open(&registry, &scope, "/missing-fixture", Some(Size::Small)));
    assert!(std::future::poll_fn(|cx| Poll::Ready(open.as_mut().poll(cx).is_pending())).await);
    scope.close();
    assert!(open.await.is_err());
    drop(permits);
    core.stop_core().await;
}
