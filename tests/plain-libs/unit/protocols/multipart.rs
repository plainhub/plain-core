use super::*;
use axum::body::Body;
use std::convert::Infallible;

fn request(body: Body, content_type: &str) -> Request {
    Request::builder()
        .header(CONTENT_TYPE, content_type)
        .body(body)
        .unwrap()
}

#[tokio::test]
async fn preserves_fragmented_boundaries_and_escaped_filenames() {
    let wire = b"preamble\r\n--plain-boundary \t\r\nContent-Disposition: form-data; filename=\"one\\\"two.txt\"; name=file\r\nContent-Type: text/plain\r\n\r\nfirst\r\n--plain-boundar\r\nlast\r\n--plain-boundary--\r\n";
    struct Fragments {
        bytes: &'static [u8],
        pending: bool,
    }

    impl Stream for Fragments {
        type Item = std::result::Result<Bytes, Infallible>;

        fn poll_next(
            mut self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            if self.pending {
                self.pending = false;
                context.waker().wake_by_ref();
                return Poll::Pending;
            }
            let Some((byte, remaining)) = self.bytes.split_first() else {
                return Poll::Ready(None);
            };
            self.bytes = remaining;
            self.pending = true;
            Poll::Ready(Some(Ok(Bytes::copy_from_slice(&[*byte]))))
        }
    }

    let stream = Fragments {
        bytes: wire,
        pending: false,
    };
    let mut form = Multipart::from_request(request(
        Body::from_stream(stream),
        "multipart/form-data; boundary=\"plain-boundary\"",
    ))
    .unwrap();
    let field = form.next_field().await.unwrap().unwrap();
    assert_eq!(field.name(), Some("file"));
    assert_eq!(field.file_name(), Some("one\"two.txt"));
    assert_eq!(field.content_type(), Some("text/plain"));
    assert_eq!(
        field.bytes().await.unwrap(),
        b"first\r\n--plain-boundar\r\nlast"[..]
    );
    assert!(form.next_field().await.unwrap().is_none());
}

#[tokio::test]
async fn rejects_invalid_boundary_truncated_body_and_default_length_overflow() {
    assert!(Multipart::from_request(request(Body::empty(), "text/plain")).is_none());
    assert!(Multipart::from_request(request(Body::empty(), "multipart/form-data")).is_none());
    for wire in [
        Body::from("--abc\r\nContent-Disposition: form-data; name=file\r\n\r\ntruncated"),
        Body::from(vec![0; 2 * 1024 * 1024 + 1]),
    ] {
        let mut form =
            Multipart::from_request(request(wire, "multipart/form-data; boundary=abc")).unwrap();
        let error = match form.next_field().await {
            Ok(Some(field)) => field.bytes().await.unwrap_err(),
            Err(error) => error,
            Ok(None) => panic!("malformed multipart unexpectedly ended successfully"),
        };
        assert_eq!(
            error.to_string(),
            "Error parsing `multipart/form-data` request"
        );
    }
}
