use super::*;
#[test]
fn quoted_parameters_case_and_invalid_bytes_keep_their_rules() {
    let mime = r#"Multipart/Form-Data; boundary="a b"; charset=UTF-8"#
        .parse::<Mime>()
        .unwrap();
    assert_eq!(mime.type_(), MULTIPART);
    assert_eq!(mime.get_param("boundary").unwrap().as_str(), "a b");
    assert_eq!(mime.get_param(CHARSET).unwrap(), UTF_8);
    assert_eq!("text/".parse::<Mime>().unwrap().as_ref(), "text/");
    for bad in [
        "text",
        "text/plain\r\nx: y",
        "text/plain; charset=\"unterminated",
    ] {
        assert!(bad.parse::<Mime>().is_err());
    }
}
