use super::*;

#[test]
fn test_replace() {
    let original = "This is a ''test''";
    let normalized = "This is a \"test\"";

    let mut n = NormalizedString::from(original);
    Replace::new("''", "\"").unwrap().normalize(&mut n).unwrap();

    assert_eq!(&n.get(), &normalized);
}

#[test]
fn test_replace_regex() {
    let original = "This     is   a         test";
    let normalized = "This is a test";

    let mut n = NormalizedString::from(original);
    Replace::new(ReplacePattern::Regex(r"\s+".into()), ' ')
        .unwrap()
        .normalize(&mut n)
        .unwrap();

    assert_eq!(&n.get(), &normalized);
}

#[test]
fn serialization() {
    let replace = Replace::new("Hello", "Hey").unwrap();
    let replace_s = r#"{"type":"Replace","pattern":{"String":"Hello"},"content":"Hey"}"#;
    assert_eq!(crate::serde_json::to_string(&replace).unwrap(), replace_s);
    assert_eq!(crate::serde_json::from_str::<Replace>(replace_s).unwrap(), replace);

    let replace = Replace::new(ReplacePattern::Regex(r"\s+".into()), ' ').unwrap();
    let replace_s = r#"{"type":"Replace","pattern":{"Regex":"\\s+"},"content":" "}"#;
    assert_eq!(crate::serde_json::to_string(&replace).unwrap(), replace_s);
    assert_eq!(crate::serde_json::from_str::<Replace>(replace_s).unwrap(), replace);
}

#[test]
fn test_replace_decode() {
    let original = vec!["hello".to_string(), "_hello".to_string()];
    let replace = Replace::new("_", " ").unwrap();
    assert_eq!(
        replace.decode_chain(original).unwrap(),
        vec!["hello", " hello"]
    );
}
