use crate::{ConstValue, Name};
use plain_libs::indexmap::IndexMap;
#[test]
fn test_macro() {
    assert_eq!(value!(1), ConstValue::Number(1.into()));
    assert_eq!(value!(1 + 2), ConstValue::Number(3.into()));
    assert_eq!(value!("abc"), ConstValue::String("abc".into()));
    assert_eq!(value!(true), ConstValue::Boolean(true));
    assert_eq!(
        value!([1, 2, 3]),
        ConstValue::List((1..=3).map(|n| ConstValue::Number(n.into())).collect())
    );
    assert_eq!(
        value!([1, 2, 3,]),
        ConstValue::List((1..=3).map(|n| ConstValue::Number(n.into())).collect())
    );
    assert_eq!(value ! ({ "a" : 10 , "b" : true }), {
        let mut map = IndexMap::new();
        map.insert(Name::new("a"), ConstValue::Number(10.into()));
        map.insert(Name::new("b"), ConstValue::Boolean(true));
        ConstValue::Object(map)
    });
}
