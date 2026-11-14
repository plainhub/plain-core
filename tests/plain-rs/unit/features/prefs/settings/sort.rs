use super::*;
use plain_libs::serde_json::{from_str, to_value};

#[test]
fn named_sort_contracts_reject_irrelevant_fields() {
    for value in ["TAKEN_AT_DESC", "TITLE_ASC", "DATE_ASC"] {
        assert!(from_str::<FileSortBy>(&format!("\"{value}\"")).is_err());
        assert!(from_str::<DocSortBy>(&format!("\"{value}\"")).is_err());
    }
    for value in ["TAKEN_AT_DESC", "NAME_ASC", "DATE_DESC"] {
        assert!(from_str::<AudioSortBy>(&format!("\"{value}\"")).is_err());
    }
    assert_eq!(from_str::<ImageSortBy>("\"TAKEN_AT_DESC\"").unwrap(), ImageSortBy::TakenAtDesc);
    assert_eq!(from_str::<VideoSortBy>("\"TAKEN_AT_DESC\"").unwrap(), VideoSortBy::TakenAtDesc);
    assert_eq!(to_value(AudioSortBy::TitleAsc).unwrap(), "TITLE_ASC");
    assert_eq!(MediaSortBy::from(AudioSortBy::TitleAsc), MediaSortBy::TitleAsc);
    assert_eq!(MediaSortBy::from(ImageSortBy::NameAsc), MediaSortBy::NameAsc);
}
