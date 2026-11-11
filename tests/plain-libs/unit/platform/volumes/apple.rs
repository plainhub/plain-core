use super::*;

#[test]
fn firmlink_paths_bind_to_the_namespace_root_with_the_same_volume_uuid() {
    let root = root_for_path(
        Path::new("/Users/fixture/media/audio.wav"),
        PathBuf::from("/System/Volumes/Data"),
        "UUID:owned-volume",
        |path| {
            assert_eq!(path, Path::new("/"));
            Ok("UUID:owned-volume".into())
        },
    )
    .unwrap();
    assert_eq!(root, Path::new("/"));
}

#[test]
fn paths_inside_the_reported_root_preserve_it_without_another_lookup() {
    let root = root_for_path(
        Path::new("/Volumes/owned/media/audio.wav"),
        PathBuf::from("/Volumes/owned"),
        "UUID:owned-volume",
        |_| panic!("Reported root already contains the path"),
    )
    .unwrap();
    assert_eq!(root, Path::new("/Volumes/owned"));
}

#[test]
fn a_namespace_root_on_another_volume_cannot_rebind_the_path() {
    assert!(
        root_for_path(
            Path::new("/Users/fixture/media"),
            PathBuf::from("/Volumes/owned"),
            "UUID:owned-volume",
            |_| Ok("UUID:other-volume".into()),
        )
        .is_err()
    );
    assert!(
        root_for_path(
            Path::new("/Users/fixture/media"),
            PathBuf::from("/Volumes/owned"),
            "UUID:owned-volume",
            |_| crate::anyhow::bail!("Volume identity unavailable"),
        )
        .is_err()
    );
}
