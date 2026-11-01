use super::*;
use std::io::Cursor;

#[tokio::test]
async fn streams_then_ends_at_eof() {
    let data = vec![7u8; 200_000]; // > 64 KiB → multiple chunks
    let mut stream = Box::pin(AsyncReadStream::new(Cursor::new(data)));
    let mut total = 0;
    let mut chunks = 0;
    while let Some(res) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
        let bytes = res.unwrap();
        assert!(bytes.len() <= 64 * 1024);
        assert_eq!(bytes[0], 7);
        total += bytes.len();
        chunks += 1;
    }
    assert_eq!(total, 200_000);
    assert!(chunks >= 3);
}

#[tokio::test]
async fn chunk_content_is_byte_exact_across_boundaries() {
    // Position-dependent pattern: any off-by-one copy bug would smear it.
    let data: Vec<u8> = (0..150_000u32).map(|i| (i % 251) as u8).collect();
    let mut stream = Box::pin(AsyncReadStream::new(Cursor::new(data.clone())));
    let mut got = Vec::new();
    while let Some(res) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
        got.extend_from_slice(&res.unwrap());
    }
    assert_eq!(got, data);
}

#[tokio::test]
async fn zero_len_reader_ends_immediately() {
    let mut stream = Box::pin(AsyncReadStream::new(Cursor::new(Vec::<u8>::new())));
    assert!(
        std::future::poll_fn(|cx| stream.as_mut().poll_next(cx))
            .await
            .is_none()
    );
}
