use super::*;
#[test]
fn var_serde() {
    let var = Value::Variable(Name::new("abc"));
    let s = plain_libs::serde_json::to_string(&var).unwrap();
    assert_eq!(s, r#"{"$var":"abc"}"#);
    assert_eq!(var, plain_libs::serde_json::from_str(&s).unwrap());
}
