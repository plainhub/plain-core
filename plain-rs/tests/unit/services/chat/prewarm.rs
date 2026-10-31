use super::*;

#[test]
fn start_receipt_requires_actual_boolean_and_no_errors() {
    assert!(!started(&json!({"data":{"startAware":false}})).unwrap());
    assert!(started(&json!({"data":{"startAware":true},"errors":[]})).unwrap());
    assert!(started(&json!({"data":{"startAware":true},"errors":[{"message":"failed"}]})).is_err());
    assert!(started(&json!({"data":{}})).is_err());
}
