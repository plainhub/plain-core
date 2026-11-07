use super::*;

#[test]
fn display_formatted_errors() {
    let expected = "Some error";

    assert_eq!(expected, format!("{}", Error::generic(expected)));

    assert_eq!(
        expected,
        format!(
            "{}",
            Error::io(io::Error::new(io::ErrorKind::Other, expected))
        )
    );
}
