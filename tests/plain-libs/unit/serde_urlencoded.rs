use super::*;
#[test]
fn form_escaping_duplicate_keys_and_float_bytes_are_preserved() {
    assert_eq!(
        to_string([("a b", "+/&汉字"), ("a b", "")]).unwrap(),
        "a+b=%2B%2F%26%E6%B1%89%E5%AD%97&a+b="
    );
    assert_eq!(
        from_str::<Vec<(String, String)>>("a+b=%2B%2F%26%E6%B1%89%E5%AD%97&a+b=").unwrap(),
        vec![("a b".into(), "+/&汉字".into()), ("a b".into(), "".into())]
    );
    assert_eq!(
        to_string([("zero", -0.0), ("float", 1.25)]).unwrap(),
        "zero=-0.0&float=1.25"
    );
    assert!(from_str::<std::collections::HashMap<String, u8>>("a=256").is_err());
}
