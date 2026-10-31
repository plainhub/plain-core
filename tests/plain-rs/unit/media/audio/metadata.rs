use super::*;

#[test]
fn unknown_artist_normalization_is_shared_without_changing_other_names() {
    assert_eq!(normalize_artist("<unknown>"), "");
    assert_eq!(normalize_artist("Artist<unknown>"), "Artist");
    assert_eq!(normalize_artist("  Artist  "), "  Artist  ");
    assert_eq!(normalize_artist("中文歌手"), "中文歌手");
}

#[test]
fn album_identity_is_stable_uses_album_artist_and_does_not_group_unknown_albums() {
    let id = album_id("Shared Album", "Album Artist", "Track One");
    assert!(id.starts_with("album:"));
    assert_eq!(
        id,
        album_id(" shared ALBUM ", " ALBUM ARTIST ", "Track Two")
    );
    assert_ne!(id, album_id("Different Album", "Album Artist", "Track One"));
    assert_ne!(
        id,
        album_id("Shared Album", "Different Artist", "Track One")
    );
    assert_eq!(
        album_id("Album", "", "Artist"),
        album_id("Album", "Artist", "Other")
    );
    assert!(album_id("   ", "Artist", "Track Artist").is_empty());
    assert_ne!(album_id("ab", "c", ""), album_id("a", "bc", ""));
}
