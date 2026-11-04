use crate::InputType;
#[test]
fn test_optional_type() {
    assert_eq!(Option::<i32>::type_name(), "Int");
    assert_eq!(Option::<i32>::qualified_type_name(), "Int");
    assert_eq!(&Option::<i32>::type_name(), "Int");
    assert_eq!(&Option::<i32>::qualified_type_name(), "Int");
}
