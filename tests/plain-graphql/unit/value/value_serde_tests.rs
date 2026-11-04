use super::*;
#[test]
fn var_serde() {
    let var = Value::Variable(Name::new("abc"));
    let s = serde_json::to_string(&var).unwrap();
    assert_eq!(s, r#"{"$var":"abc"}"#);
    assert_eq!(var, serde_json::from_str(&s).unwrap());
}
