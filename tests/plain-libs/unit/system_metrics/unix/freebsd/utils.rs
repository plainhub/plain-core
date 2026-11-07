use super::c_buf_to_utf8_string;

#[test]
fn bounded_char_arrays_preserve_utf8_and_stop_at_the_first_nul() {
    let bytes: Vec<libc::c_char> = "磁盘 😀\0ignored"
        .bytes()
        .map(|byte| byte as libc::c_char)
        .collect();
    assert_eq!(c_buf_to_utf8_string(&bytes).as_deref(), Some("磁盘 😀"));
    assert_eq!(c_buf_to_utf8_string(&[65, 66]).as_deref(), Some("AB"));
    assert_eq!(c_buf_to_utf8_string(&[]).as_deref(), Some(""));
    assert_eq!(c_buf_to_utf8_string(&[255u8 as libc::c_char]), None);
}
