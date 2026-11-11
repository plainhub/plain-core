use super::*;
#[test]
fn static_parameter_and_catchall_routes_keep_precedence_and_borrows() {
    let mut router = Router::new();
    router.insert("/a/fixed", 1).unwrap();
    router.insert("/a/:key", 2).unwrap();
    router.insert("/files/*path", 3).unwrap();
    assert_eq!(*router.at("/a/fixed").unwrap().value, 1);
    let path = String::from("/a/value");
    let matched = router.at(&path).unwrap();
    assert_eq!(*matched.value, 2);
    let value = matched.params.get("key").unwrap();
    assert_eq!(value, "value");
    assert!(path.as_bytes().as_ptr_range().contains(&value.as_ptr()));
    assert_eq!(
        router.at("/files/a/b").unwrap().params.get("path"),
        Some("a/b")
    );
    assert!(router.insert("/a/:other", 4).is_err());
    assert!(router.at("/a").is_err());
}
