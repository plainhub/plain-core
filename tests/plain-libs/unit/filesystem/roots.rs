use super::*;

#[test]
fn nested_roots_are_pruned_by_components_without_losing_named_siblings() {
    let library = PathBuf::from("library");
    let sibling = PathBuf::from("library-backup");
    assert_eq!(
        prune_nested(
            vec![
                library.join("photos"),
                sibling.join("photos"),
                library.clone(),
                PathBuf::new(),
                sibling.clone(),
                library,
            ],
            |_| false,
        ),
        [PathBuf::from("library"), sibling]
    );
}

#[test]
fn independent_nested_roots_are_retained_once_and_cover_their_children() {
    let home = PathBuf::from("home");
    let cloud = home.join("cloud");
    assert_eq!(
        prune_nested(
            vec![
                cloud.join("documents"),
                cloud.clone(),
                home.clone(),
                cloud.clone(),
                home.join("documents"),
            ],
            |path| path == cloud,
        ),
        [home, cloud]
    );
}

#[test]
fn stable_order_does_not_depend_on_source_discovery_order() {
    let paths = vec![
        PathBuf::from("external/photos"),
        PathBuf::from("home/documents"),
        PathBuf::from("external"),
        PathBuf::from("home"),
    ];
    let expected = vec![PathBuf::from("external"), PathBuf::from("home")];
    for offset in 0..paths.len() {
        let mut paths = paths.clone();
        paths.rotate_left(offset);
        assert_eq!(prune_nested(paths.clone(), |_| false), expected);
        paths.reverse();
        assert_eq!(prune_nested(paths, |_| false), expected);
    }
}

#[test]
fn directory_existence_and_canonicalization_remain_with_the_source_adapter() {
    let path = PathBuf::from("not-an-opened-directory");
    assert_eq!(prune_nested(vec![path.clone()], |_| false), [path]);
}
