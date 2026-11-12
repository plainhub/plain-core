use super::*;
use bytes::{Buf, Bytes};
use tokio::io::AsyncWrite;
#[path = "../support/poll.rs"]
mod support;

struct Writer {
    vectored: bool,
    pending: bool,
    fail: bool,
    limit: usize,
    bytes: Vec<u8>,
    calls: usize,
    vector_calls: usize,
}
impl AsyncWrite for Writer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.calls += 1;
        if self.pending {
            self.pending = false;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if self.fail {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "synthetic pipe",
            )));
        }
        let n = bytes.len().min(self.limit);
        self.bytes.extend_from_slice(&bytes[..n]);
        Poll::Ready(Ok(n))
    }
    fn is_write_vectored(&self) -> bool {
        self.vectored
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bytes: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.vector_calls += 1;
        let mut written = 0;
        for bytes in bytes {
            let n = bytes.len().min(self.limit - written);
            self.bytes.extend_from_slice(&bytes[..n]);
            written += n;
            if written == self.limit {
                break;
            }
        }
        Poll::Ready(Ok(written))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
fn writer(vectored: bool) -> Writer {
    Writer {
        vectored,
        pending: false,
        fail: false,
        limit: 5,
        bytes: Vec::new(),
        calls: 0,
        vector_calls: 0,
    }
}

#[test]
fn partial_scalar_and_vectored_writes_advance_exactly_the_reported_bytes() {
    let (_, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    for vectored in [false, true] {
        let mut sink = writer(vectored);
        let mut bytes = Bytes::from_static(b"first").chain(Bytes::from_static(b"second"));
        let mut total = 0;
        while bytes.has_remaining() {
            let Poll::Ready(Ok(n)) = poll_write_buf(Pin::new(&mut sink), &mut cx, &mut bytes)
            else {
                panic!("write failed");
            };
            total += n;
            assert_eq!(bytes.remaining(), 11 - total);
        }
        assert_eq!(sink.bytes, b"firstsecond");
        assert_eq!(sink.vector_calls != 0, vectored);
        assert_eq!(sink.calls != 0, !vectored);
        let before = sink.vector_calls + sink.calls;
        assert!(matches!(
            poll_write_buf(Pin::new(&mut sink), &mut cx, &mut bytes),
            Poll::Ready(Ok(0))
        ));
        assert_eq!(sink.vector_calls + sink.calls, before);
    }
}

#[test]
fn pending_error_and_zero_writes_never_advance_the_buffer() {
    let (count, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    let mut sink = writer(false);
    let mut bytes = Bytes::from_static(b"payload");
    let ptr = bytes.as_ptr();
    sink.pending = true;
    assert!(poll_write_buf(Pin::new(&mut sink), &mut cx, &mut bytes).is_pending());
    assert_eq!(count.count(), 1);
    sink.fail = true;
    assert!(
        matches!(poll_write_buf(Pin::new(&mut sink), &mut cx, &mut bytes), Poll::Ready(Err(ref e)) if e.kind() == io::ErrorKind::BrokenPipe)
    );
    sink.fail = false;
    sink.limit = 0;
    assert!(matches!(
        poll_write_buf(Pin::new(&mut sink), &mut cx, &mut bytes),
        Poll::Ready(Ok(0))
    ));
    assert_eq!(bytes.as_ptr(), ptr);
    assert_eq!(&bytes[..], b"payload");
}

#[test]
fn read_buffer_pointer_validation_is_retained() {
    struct ChangesPointer;
    impl AsyncRead for ChangesPointer {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            *buf = ReadBuf::new(&mut []);
            Poll::Ready(Ok(()))
        }
    }
    let (_, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    let mut data = bytes::BytesMut::with_capacity(16);
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = poll_read_buf(Pin::new(&mut ChangesPointer), &mut cx, &mut data);
    }));
    assert!(error.is_err());
    assert!(data.is_empty());
}
