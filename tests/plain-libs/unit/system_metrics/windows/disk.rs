use super::*;

#[test]
fn bounded_volume_names_require_a_terminator() {
    assert_eq!(from_zero_terminated(&[65, 0, 66]), Some(vec![65, 0]));
    assert_eq!(from_zero_terminated(&[0]), Some(vec![0]));
    assert_eq!(from_zero_terminated(&[65]), None);
    assert_eq!(from_zero_terminated(&[]), None);
}

#[test]
fn volume_multi_strings_preserve_each_path_and_reject_partial_data() {
    assert_eq!(
        volume_paths(&[65, 0, 66, 0, 0]),
        Some(vec![vec![65, 0], vec![66, 0]])
    );
    assert_eq!(volume_paths(&[0, 0]), Some(Vec::new()));
    assert_eq!(volume_paths(&[65, 0, 66]), None);
    assert_eq!(volume_paths(&[65, 0]), None);
    assert_eq!(volume_paths(&[]), None);
}
