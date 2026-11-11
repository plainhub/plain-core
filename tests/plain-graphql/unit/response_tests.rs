use super::*;
#[test]
fn test_response_errors_serialized_before_data() {
    let mut resp = Response::new(Value::Boolean(true));
    resp.errors.push(ServerError::new("Test error", None));
    assert_eq!(
        plain_libs::serde_json::to_string(&resp).unwrap(),
        r#"{"errors":[{"message":"Test error"}],"data":true}"#
    );
}
