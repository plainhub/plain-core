use super::*;
#[test]
fn fragmented_header_parse_borrows_input_and_rejects_control_characters() {
    let input = b"GET /path HTTP/1.1\r\nHost: example.test\r\n\r\nbody";
    for split in 0..input.len() - 4 {
        let mut headers = [EMPTY_HEADER; 4];
        let mut request = Request::new(&mut headers);
        assert!(matches!(
            request.parse(&input[..split]),
            Ok(Status::Partial)
        ));
    }
    let mut headers = [EMPTY_HEADER; 4];
    let mut request = Request::new(&mut headers);
    let used = request.parse(input).unwrap().unwrap();
    assert_eq!(&input[used..], b"body");
    assert!(
        input
            .as_ptr_range()
            .contains(&request.headers[0].value.as_ptr())
    );
    for bad in [
        b"GET / HTTP/1.1\r\nBad Name: value\r\n\r\n".as_slice(),
        b"GET / HTTP/1.1\r\nX: a\0b\r\n\r\n",
    ] {
        let mut headers = [EMPTY_HEADER; 4];
        assert!(Request::new(&mut headers).parse(bad).is_err());
    }
}
