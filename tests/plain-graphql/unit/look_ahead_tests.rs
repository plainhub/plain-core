use crate::*;
#[tokio::test]
async fn test_look_ahead() {
    #[derive(SimpleObject)]
    #[graphql(internal)]
    struct Detail {
        c: i32,
        d: i32,
    }
    #[derive(SimpleObject)]
    #[graphql(internal)]
    struct MyObj {
        a: i32,
        b: i32,
        detail: Detail,
    }
    struct Query;
    #[Object(internal)]
    impl Query {
        async fn obj(&self, ctx: &Context<'_>, n: i32) -> MyObj {
            if ctx.look_ahead().field("a").exists() {
                assert_eq!(n, 1);
            } else if ctx.look_ahead().field("detail").field("c").exists()
                && ctx.look_ahead().field("detail").field("d").exists()
            {
                assert_eq!(n, 2);
            } else if ctx.look_ahead().field("detail").field("c").exists() {
                assert_eq!(n, 3);
            } else {
                assert_eq!(n, 4);
            }
            MyObj {
                a: 0,
                b: 0,
                detail: Detail { c: 0, d: 0 },
            }
        }
    }
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 1) {
                a
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 1) {
                k:a
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 3) {
                detail {
                    c
                }
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 2) {
                detail {
                    d
                }

                detail {
                    c
                }
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 4) {
                b
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 1) {
                ... {
                    a
                }
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 3) {
                ... {
                    detail {
                        c
                    }
                }
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 2) {
                ... {
                    detail {
                        d
                    }

                    detail {
                        c
                    }
                }
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 1) {
                ... A
            }
        }

        fragment A on MyObj {
            a
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 3) {
                ... A
            }
        }

        fragment A on MyObj {
            detail {
                c
            }
        }"#,
            )
            .await
            .is_ok()
    );
    assert!(
        schema
            .execute(
                r#"{
            obj(n: 2) {
                ... A
                ... B
            }
        }

        fragment A on MyObj {
            detail {
                d
            }
        }

        fragment B on MyObj {
            detail {
                c
            }
        }"#,
            )
            .await
            .is_ok()
    );
}
