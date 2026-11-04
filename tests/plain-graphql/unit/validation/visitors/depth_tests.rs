use super::*;
use crate::{
    EmptyMutation, EmptySubscription, Object, Schema, parser::parse_query, validation::visit,
};
struct Query;
struct MyObj;
#[Object(internal)]
impl MyObj {
    async fn a(&self) -> i32 {
        validation_fixture()
    }
    async fn b(&self) -> i32 {
        validation_fixture()
    }
    async fn c(&self) -> MyObj {
        validation_fixture()
    }
}
#[Object(internal)]
impl Query {
    async fn value(&self) -> i32 {
        validation_fixture()
    }
    async fn obj(&self) -> MyObj {
        validation_fixture()
    }
}
fn check_depth(query: &str, expect_depth: usize) {
    let registry = Schema::<Query, EmptyMutation, EmptySubscription>::create_registry();
    let doc = parse_query(query).unwrap();
    let mut ctx = VisitorContext::new(&registry, &doc, None, None);
    let mut depth = 0;
    let mut depth_calculate = DepthCalculate::new(&mut depth);
    visit(&mut depth_calculate, &mut ctx, &doc);
    assert_eq!(depth, expect_depth);
}
#[test]
fn depth() {
    check_depth(
        r#"{
            value #1
        }"#,
        1,
    );
    check_depth(
        r#"
        {
            obj { #1
                a b #2
            }
        }"#,
        2,
    );
    check_depth(
        r#"
        {
            obj { # 1
                a b c { # 2
                    a b c { # 3
                        a b # 4
                    }
                }
            }
        }"#,
        4,
    );
    check_depth(
        r#"
        fragment A on MyObj {
            a b ... A2 #2
        }

        fragment A2 on MyObj {
            obj {
                a #3
            }
        }

        query {
            obj { # 1
                ... A
            }
        }"#,
        3,
    );
    check_depth(
        r#"
        {
            obj { # 1
                ... on MyObj {
                    a b #2
                    ... on MyObj {
                        obj {
                            a #3
                        }
                    }
                }
            }
        }"#,
        3,
    );
}

fn validation_fixture<T>() -> T {
    panic!("validation fixtures must not execute")
}
