use super::*;

#[test]
fn uuid_v7_preserves_unix_milliseconds_version_variant_and_time_order() {
    for millis in [0, 1, 1_793_836_800_123, (1u64 << 48) - 1] {
        let id = from_unix_millis(millis);
        let mut timestamp = [0; 8];
        timestamp[2..].copy_from_slice(&id.as_bytes()[..6]);
        assert_eq!(u64::from_be_bytes(timestamp), millis);
        assert_eq!(id.as_bytes()[6] >> 4, 7);
        assert_eq!(id.as_bytes()[8] & 0xc0, 0x80);
    }
    assert!(from_unix_millis(1_793_836_800_123) < from_unix_millis(1_793_836_800_124));
    let ids = (0..256)
        .map(|_| from_unix_millis(1_793_836_800_123))
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 256);
}

#[test]
fn generated_record_ids_contain_the_current_timestamp() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let id = Uuid::parse_str(&generate()).unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let mut bytes = [0; 8];
    bytes[2..].copy_from_slice(&id.as_bytes()[..6]);
    assert!((before..=after).contains(&(u64::from_be_bytes(bytes) as u128)));
    assert_eq!(id.as_bytes()[6] >> 4, 7);
}

#[cfg(all(feature = "chat", feature = "library"))]
#[test]
fn database_record_creators_use_uuid_v7_and_updates_preserve_identity() {
    use crate::infrastructure::db::bookmark::{DBookmark, DBookmarkGroup};
    use crate::infrastructure::db::{DChannel, DChat, Db};
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let note = crate::features::notes::create(&db, "Note", "Content").unwrap();
    let updated = crate::features::notes::update(&db, &note.id, "Updated", "Content").unwrap();
    assert_eq!(note.id, updated.id);
    let tag = crate::features::tags::create_tag(&db, 0, "Tag").unwrap();
    let (clipboard, _) =
        crate::features::clipboard::record(&db, "Text", "test", "", false).unwrap();
    let ids = [
        note.id,
        tag.id,
        clipboard.id,
        DBookmark::new("https://example.com", "").id,
        DBookmarkGroup::new("Group").id,
        DChannel::new("Channel", "owner").id,
        DChat::new("from", "to", "", "content").id,
    ];
    for value in ids {
        let id = Uuid::parse_str(&value).unwrap();
        assert_eq!(id.as_bytes()[6] >> 4, 7, "{value}");
        assert_eq!(id.as_bytes()[8] & 0xc0, 0x80);
    }
}
