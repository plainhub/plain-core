use crate::audio_tags::id3::v2::util::pairs::set_number;
use crate::audio_tags::tag::{ItemKey, ItemValue, TagItem};

#[test]
fn whitespace_in_number() {
    let item = TagItem::new(
        ItemKey::TrackNumber,
        ItemValue::Text(String::from("  12  ")),
    );
    set_number(&item, |number| assert_eq!(number, 12));
}

#[test]
fn empty_number_string() {
    let item = TagItem::new(ItemKey::TrackNumber, ItemValue::Text(String::new()));
    set_number(&item, |_| unreachable!("Should not be called"));

    // Also with whitespace only strings
    let item = TagItem::new(
        ItemKey::TrackNumber,
        ItemValue::Text(String::from("        ")),
    );
    set_number(&item, |_| unreachable!("Should not be called"));
}
