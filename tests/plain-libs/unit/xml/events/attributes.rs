use super::*;
use crate::xml::{Reader, events::Event};

#[test]
fn large_start_tags_use_hashed_duplicate_checks_and_keep_original_error_positions() {
    let mut xml = String::from("<r");
    for index in 0..4096 {
        xml.push_str(&format!(" a{index}='value'"));
    }
    xml.push_str(" a0='duplicate'/>");
    let first = xml.find("a0='").unwrap() - 1;
    let duplicate = xml.rfind("a0='").unwrap() - 1;
    let mut reader = Reader::from_str(&xml);
    let Event::Empty(element) = reader.read_event().unwrap() else {
        panic!("expected empty element")
    };
    let mut attrs = element.attributes();
    for index in 0..4096 {
        let attr = attrs.next().unwrap().unwrap();
        assert_eq!(attr.key.0, format!("a{index}"));
        assert_eq!(attrs.state.key_hashes.is_some(), index >= 32);
    }
    assert!(matches!(
        attrs.next().unwrap(),
        Err(AttrError::Duplicated(at, previous)) if at == duplicate && previous == first
    ));
}
