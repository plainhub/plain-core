use super::*;
use plain_graphql::{EmptySubscription, Schema};
use plain_libs::serde_json::json;

#[tokio::test]
async fn favorite_alias_requires_an_existing_favorite() {
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    favorite_folders::add_full_path(&db, "/fixtures", "/fixtures/saved").unwrap();
    let schema = Schema::build(FavoriteFolderQuery, FavoriteFolderMutation, EmptySubscription)
        .data(db.clone())
        .finish();
    let missing = schema.execute(r#"mutation {
        setFavoriteFolderAlias(fullPath:"/fixtures/missing", alias:"Missing") { fullPath alias }
    }"#).await;
    assert_eq!(missing.errors.len(), 1);
    assert_eq!(missing.errors[0].message, "Favorite folder not found");
    assert_eq!(favorite_folders::list(&db).unwrap().len(), 1);
    assert!(favorite_folders::list(&db).unwrap()[0].alias.is_none());

    for (alias, expected) in [("Saved", json!("Saved")), ("", json!(null))] {
        let response = schema.execute(format!(
            "mutation {{ setFavoriteFolderAlias(fullPath:\"/fixtures/saved\", alias:{}) {{ fullPath alias }} }}",
            plain_libs::serde_json::to_string(alias).unwrap()
        )).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["setFavoriteFolderAlias"],
            json!([{"fullPath":"/fixtures/saved", "alias":expected}]));
    }
}
