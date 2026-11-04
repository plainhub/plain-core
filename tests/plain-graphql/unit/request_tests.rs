use crate::*;
#[test]
fn test_request() {
    let request: Request = from_value(value ! ({ "query" : "{ a b c }" })).unwrap();
    assert!(request.variables.is_empty());
    assert!(request.operation_name.is_none());
    assert_eq!(request.query, "{ a b c }");
}
#[test]
fn test_request_with_operation_name() {
    let request: Request =
        from_value(value ! ({ "query" : "{ a b c }" , "operationName" : "a" })).unwrap();
    assert!(request.variables.is_empty());
    assert_eq!(request.operation_name.as_deref(), Some("a"));
    assert_eq!(request.query, "{ a b c }");
}
#[test]
fn test_request_with_variables() {
    let request : Request = from_value (value ! ({ "query" : "{ a b c }" , "variables" : { "v1" : 100 , "v2" : [1 , 2 , 3] , "v3" : "str" , } })) . unwrap () ;
    assert_eq!(
        request.variables.into_value(),
        value ! ({ "v1" : 100 , "v2" : [1 , 2 , 3] , "v3" : "str" , })
    );
    assert!(request.operation_name.is_none());
    assert_eq!(request.query, "{ a b c }");
}
#[test]
fn test_deserialize_request_with_empty_object_variables() {
    let request: Request =
        from_value(value ! ({ "query" : "{ a b c }" , "variables" : { } })).unwrap();
    assert!(request.operation_name.is_none());
    assert!(request.variables.is_empty());
}
#[test]
fn test_deserialize_request_with_empty_array_variables() {
    let error: DeserializerError =
        from_value::<Request>(value ! ({ "query" : "{ a b c }" , "variables" : [] })).unwrap_err();
    assert_eq!(error.to_string(), "invalid type: sequence, expected a map");
}
#[test]
fn test_deserialize_request_with_null_variables() {
    let request: Request =
        from_value(value ! ({ "query" : "{ a b c }" , "variables" : null })).unwrap();
    assert!(request.operation_name.is_none());
    assert!(request.variables.is_empty());
}
