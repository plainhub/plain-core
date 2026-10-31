#[tokio::test]
async fn main_mutation_and_resource_store_share_one_runtime_and_keep_other_runtimes_isolated() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first = crate::server::ContentServer::open(first.path()).unwrap();
    let second = crate::server::ContentServer::open(second.path()).unwrap();
    let response = first
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            r#"mutation { setTempValue(key:"selection",value:"[]") { key value } }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(
        second
            .runtime_state()
            .temp_values
            .take("selection")
            .is_none()
    );
    assert_eq!(
        first.runtime_state().temp_values.take("selection"),
        Some("[]".into())
    );
    assert!(
        first
            .runtime_state()
            .temp_values
            .take("selection")
            .is_none()
    );
    first.stop_core().await;
    second.stop_core().await;
}
