use super::*;
thread_local! { static AUTH_BUFFER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub(super) fn record_auth_buffer(pointer: usize) {
    AUTH_BUFFER.set(pointer);
}
#[test]
fn basic_auth_uses_one_output_and_keeps_header_sensitivity() {
    for (user, password, expected) in [
        ("u", Some("p"), "Basic dTpw"),
        ("", None, "Basic Og=="),
        ("user", Some("a:b"), "Basic dXNlcjphOmI="),
    ] {
        let value = encode_basic_auth(user, password);
        assert_eq!(value.to_str().unwrap(), expected);
        assert!(value.is_sensitive());
        assert_eq!(value.as_bytes().as_ptr() as usize, AUTH_BUFFER.get());
    }
}
