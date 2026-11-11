use super::*;
use serde::Deserialize;
#[derive(Debug, Deserialize)]
struct Item {
    number: u8,
}
#[test]
fn nested_deserialization_failure_retains_its_path() {
    let mut de = crate::serde_json::Deserializer::from_str(r#"[{"number":300}]"#);
    let error = deserialize::<_, Vec<Item>>(&mut de).unwrap_err();
    assert_eq!(error.path().to_string(), "[0].number");
    assert!(error.inner().is_data());
    let mut de = crate::serde_json::Deserializer::from_str(r#"[{"number":7}]"#);
    assert_eq!(deserialize::<_, Vec<Item>>(&mut de).unwrap()[0].number, 7);
}
