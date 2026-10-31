use super::*;

#[test]
fn values_are_consumed_once_overwritten_and_shared_with_clones() {
    let values = TempValues::default();
    assert!(values.take("selection").is_none());
    values.set("selection".into(), "old".into());
    values.clone().set("selection".into(), "current".into());
    assert_eq!(values.clone().take("selection"), Some("current".into()));
    assert!(values.take("selection").is_none());
}
