use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[tokio::test]
async fn test_json_type() {
    #[derive(Serialize, Deserialize)]
    struct MyStruct {
        a: i32,
        b: i32,
        c: HashMap<String, i32>,
    }
    struct Query;
    #[Object(internal)]
    impl Query {
        async fn obj(&self, input: Json<MyStruct>) -> Json<MyStruct> {
            input
        }
    }
    let query = r#"{ obj(input: { a: 1, b: 2, c: { a: 11, b: 22 } } ) }"#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value ! ({ "obj" : { "a" : 1 , "b" : 2 , "c" : { "a" : 11 , "b" : 22 } } })
    );
}
#[tokio::test]
async fn test_json_type_for_serialize_only() {
    #[derive(Serialize)]
    struct MyStruct {
        a: i32,
        b: i32,
        c: HashMap<String, i32>,
    }
    struct Query;
    #[Object(internal)]
    impl Query {
        async fn obj(&self) -> Json<MyStruct> {
            MyStruct {
                a: 1,
                b: 2,
                c: {
                    let mut values = HashMap::new();
                    values.insert("a".to_string(), 11);
                    values.insert("b".to_string(), 22);
                    values
                },
            }
            .into()
        }
    }
    let query = r#"{ obj }"#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value ! ({ "obj" : { "a" : 1 , "b" : 2 , "c" : { "a" : 11 , "b" : 22 } } })
    );
}
