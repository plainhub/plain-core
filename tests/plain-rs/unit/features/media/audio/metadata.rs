use super::*;

#[test]
fn unknown_artist_normalization_is_shared_without_changing_other_names() {
    assert_eq!(normalize_artist("<unknown>"), "");
    assert_eq!(normalize_artist("Artist<unknown>"), "Artist");
    assert_eq!(normalize_artist("  Artist  "), "  Artist  ");
    assert_eq!(normalize_artist("中文歌手"), "中文歌手");
}
