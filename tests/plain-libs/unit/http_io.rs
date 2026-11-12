use super::*;
use bytes::BytesMut;
use futures_core::Stream;
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};
#[path = "../support/poll.rs"]
mod support;

fn frame(len: usize) -> Vec<u8> {
    let mut bytes = vec![
        (len >> 16) as u8,
        (len >> 8) as u8,
        len as u8,
        0,
        0,
        0,
        0,
        0,
        1,
    ];
    bytes.extend((0..len).map(|n| ((n * 17 + len) % 251) as u8));
    bytes
}
enum Step {
    Data(Vec<u8>),
    Pending,
    Eof,
    Error,
}
struct Reader {
    steps: VecDeque<Step>,
    calls: Arc<AtomicUsize>,
}
impl Reader {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl AsyncRead for Reader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.steps.pop_front().unwrap_or(Step::Eof) {
            Step::Data(mut bytes) => {
                let n = bytes.len().min(buf.remaining());
                buf.put_slice(&bytes[..n]);
                if n < bytes.len() {
                    bytes.drain(..n);
                    self.steps.push_front(Step::Data(bytes));
                }
                Poll::Ready(Ok(()))
            }
            Step::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Step::Eof => Poll::Ready(Ok(())),
            Step::Error => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "synthetic reset",
            ))),
        }
    }
}

#[test]
fn codec_splits_frames_without_copying_and_preserves_consecutive_bytes() {
    let a = frame(11);
    let b = frame(17);
    let mut bytes = BytesMut::with_capacity(a.len() + b.len() + 8);
    bytes.extend_from_slice(&a);
    bytes.extend_from_slice(&b);
    let ptr = bytes.as_ptr();
    let mut codec = LengthDelimitedCodec::new();
    let first = codec.decode(&mut bytes).unwrap().unwrap();
    assert_eq!(first.as_ptr(), ptr);
    assert_eq!(&first[..], &a);
    let second = codec.decode(&mut bytes).unwrap().unwrap();
    assert_eq!(second.as_ptr(), ptr.wrapping_add(a.len()));
    assert_eq!(&second[..], &b);
    assert!(codec.decode(&mut bytes).unwrap().is_none());
}

#[test]
fn every_fragment_boundary_and_zero_payload_frame_is_preserved() {
    let (count, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    for len in [0, 1, 7, 64, 1024] {
        let expected = frame(len);
        for split in 0..=expected.len() {
            let mut steps = Vec::new();
            if split != 0 {
                steps.push(Step::Data(expected[..split].to_vec()));
            }
            steps.push(Step::Pending);
            if split < expected.len() {
                steps.push(Step::Data(expected[split..].to_vec()));
            }
            let mut reader = FramedRead::new(Reader::new(steps), LengthDelimitedCodec::new());
            let mut result = None;
            for _ in 0..3 {
                if let Poll::Ready(Some(Ok(bytes))) = Pin::new(&mut reader).poll_next(&mut cx) {
                    result = Some(bytes);
                    break;
                }
            }
            assert_eq!(&result.unwrap()[..], &expected, "fragment boundary {split}");
        }
    }
    assert!(count.count() > 0);
}

#[test]
fn oversized_payload_is_rejected_before_reserving_or_consuming_input() {
    let mut bytes = BytesMut::from(&[255, 255, 255][..]);
    let ptr = bytes.as_ptr();
    let capacity = bytes.capacity();
    let mut codec = LengthDelimitedCodec::new();
    codec.set_max_frame_length(16 * 1024);
    let error = codec.decode(&mut bytes).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.get_ref().unwrap().is::<LengthDelimitedCodecError>());
    assert_eq!(bytes.as_ptr(), ptr);
    assert_eq!(bytes.capacity(), capacity);
    assert_eq!(&bytes[..], &[255, 255, 255]);
}

#[test]
fn limit_changes_apply_to_the_next_header_and_not_an_inflight_frame() {
    let data = frame(4);
    let mut bytes = BytesMut::from(&data[..3]);
    let mut codec = LengthDelimitedCodec::new();
    codec.set_max_frame_length(4);
    assert!(codec.decode(&mut bytes).unwrap().is_none());
    codec.set_max_frame_length(0);
    assert_eq!(codec.max_frame_length(), 0);
    bytes.extend_from_slice(&data[3..]);
    assert_eq!(&codec.decode(&mut bytes).unwrap().unwrap()[..], &data);
    bytes.extend_from_slice(&frame(1));
    assert_eq!(
        codec.decode(&mut bytes).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn every_truncated_frame_errors_once_and_retains_bytes_for_resumption() {
    let expected = frame(13);
    let (_, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    for split in 1..expected.len() {
        let input = Reader::new([
            Step::Data(expected[..split].to_vec()),
            Step::Eof,
            Step::Data(expected[split..].to_vec()),
        ]);
        let calls = input.calls.clone();
        let mut reader = FramedRead::new(input, LengthDelimitedCodec::new());
        let Poll::Ready(Some(Err(error))) = Pin::new(&mut reader).poll_next(&mut cx) else {
            panic!("truncated frame was accepted");
        };
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "bytes remaining on stream");
        let before = calls.load(Ordering::SeqCst);
        assert!(matches!(
            Pin::new(&mut reader).poll_next(&mut cx),
            Poll::Ready(None)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), before);
        let Poll::Ready(Some(Ok(actual))) = Pin::new(&mut reader).poll_next(&mut cx) else {
            panic!("resume lost the frame");
        };
        assert_eq!(&actual[..], &expected);
    }
}

#[test]
fn clean_eof_and_io_errors_keep_the_original_pause_and_resume_contract() {
    let expected = frame(3);
    let (_, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    let input = Reader::new([
        Step::Eof,
        Step::Data(expected.clone()),
        Step::Error,
        Step::Data(expected.clone()),
    ]);
    let mut reader = FramedRead::new(input, LengthDelimitedCodec::new());
    assert!(matches!(
        Pin::new(&mut reader).poll_next(&mut cx),
        Poll::Ready(None)
    ));
    assert!(
        matches!(Pin::new(&mut reader).poll_next(&mut cx), Poll::Ready(Some(Ok(ref bytes))) if bytes.as_ref() == expected)
    );
    assert!(
        matches!(Pin::new(&mut reader).poll_next(&mut cx), Poll::Ready(Some(Err(ref e))) if e.kind() == io::ErrorKind::ConnectionReset)
    );
    assert!(matches!(
        Pin::new(&mut reader).poll_next(&mut cx),
        Poll::Ready(None)
    ));
    assert!(
        matches!(Pin::new(&mut reader).poll_next(&mut cx), Poll::Ready(Some(Ok(ref bytes))) if bytes.as_ref() == expected)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multiplexed_h2_echo_preserves_all_bytes_after_canceling_an_inflight_stream() {
    use crate::{
        http_body_util::{BodyExt, Full},
        hyper::{Request, Response, body::Incoming, service::service_fn},
        hyper_util::rt::{TokioExecutor, TokioIo},
    };
    use bytes::Bytes;
    use std::{convert::Infallible, sync::Mutex};
    let (client_io, server_io) = tokio::io::duplex(257);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let started = Arc::new(Mutex::new(Some(started_tx)));
    let release = Arc::new(tokio::sync::Notify::new());
    let served = Arc::new(AtomicUsize::new(0));
    let service = service_fn({
        let release = release.clone();
        let served = served.clone();
        move |request: Request<Incoming>| {
            let started = started.clone();
            let release = release.clone();
            let served = served.clone();
            async move {
                let path = request.uri().path().to_owned();
                if path == "/cancel" {
                    let notification = { started.lock().unwrap().take() };
                    if let Some(notification) = notification {
                        notification.send(()).unwrap();
                    }
                    release.notified().await;
                }
                let body = request.into_body().collect().await.unwrap().to_bytes();
                if path != "/cancel" {
                    served.fetch_add(1, Ordering::SeqCst);
                }
                Ok::<_, Infallible>(
                    Response::builder()
                        .header("x-echo-path", path)
                        .body(Full::new(body))
                        .unwrap(),
                )
            }
        }
    });
    let server = crate::hyper::server::conn::http2::Builder::new(TokioExecutor::new())
        .serve_connection(TokioIo::new(server_io), service);
    let server = tokio::spawn(server);
    let (sender, connection) =
        crate::hyper::client::conn::http2::handshake(TokioExecutor::new(), TokioIo::new(client_io))
            .await
            .unwrap();
    let connection = tokio::spawn(connection);
    let mut canceled_sender = sender.clone();
    let canceled = tokio::spawn(async move {
        canceled_sender
            .send_request(
                Request::builder()
                    .uri("/cancel")
                    .body(Full::new(Bytes::new()))
                    .unwrap(),
            )
            .await
    });
    started_rx.await.unwrap();
    canceled.abort();
    assert!(canceled.await.unwrap_err().is_cancelled());
    release.notify_waiters();
    let mut requests = Vec::new();
    for index in 0usize..8 {
        let mut sender = sender.clone();
        requests.push(tokio::spawn(async move {
            let path = format!("/echo/{index}");
            let bytes = Bytes::from(
                (0..(65 * 1024 + index))
                    .map(|n| ((n * 13 + index) % 251) as u8)
                    .collect::<Vec<_>>(),
            );
            let response = sender
                .send_request(
                    Request::builder()
                        .uri(path.clone())
                        .body(Full::new(bytes.clone()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.headers()["x-echo-path"].to_str().unwrap(), path);
            assert_eq!(
                response.into_body().collect().await.unwrap().to_bytes(),
                bytes
            );
        }));
    }
    for request in requests {
        request.await.unwrap();
    }
    assert_eq!(served.load(Ordering::SeqCst), 8);
    drop(sender);
    connection.await.unwrap().unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn h2_connect_half_close_drains_the_tunnel_before_signaling_eof() {
    use crate::{
        http_body_util::Full,
        hyper::{Method, Request, Response, body::Incoming, service::service_fn},
        hyper_util::rt::{TokioExecutor, TokioIo},
    };
    use bytes::Bytes;
    use std::convert::Infallible;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (client_io, server_io) = tokio::io::duplex(257);
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let finished = Arc::new(std::sync::Mutex::new(Some(finished_tx)));
    let expected: Vec<u8> = (0..(96 * 1024))
        .map(|n| ((n * 19 + 3) % 251) as u8)
        .collect();
    let service = service_fn({
        let expected = expected.clone();
        move |mut request: Request<Incoming>| {
            assert_eq!(request.method(), Method::CONNECT);
            let upgrade = crate::hyper::upgrade::on(&mut request);
            let finished = finished.clone();
            let expected = expected.clone();
            tokio::spawn(async move {
                let mut tunnel = TokioIo::new(upgrade.await.unwrap());
                let mut received = Vec::new();
                tunnel.read_to_end(&mut received).await.unwrap();
                assert_eq!(received, expected);
                tunnel.write_all(&received).await.unwrap();
                tunnel.shutdown().await.unwrap();
                finished.lock().unwrap().take().unwrap().send(()).unwrap();
            });
            async { Ok::<_, Infallible>(Response::new(Full::new(Bytes::new()))) }
        }
    });
    let server = crate::hyper::server::conn::http2::Builder::new(TokioExecutor::new())
        .serve_connection(TokioIo::new(server_io), service);
    let server = tokio::spawn(server);
    let (mut sender, connection) =
        crate::hyper::client::conn::http2::handshake(TokioExecutor::new(), TokioIo::new(client_io))
            .await
            .unwrap();
    let connection = tokio::spawn(connection);
    let mut response = sender
        .send_request(
            Request::builder()
                .method(Method::CONNECT)
                .uri("synthetic.invalid:443")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), crate::http::StatusCode::OK);
    let mut tunnel = TokioIo::new(crate::hyper::upgrade::on(&mut response).await.unwrap());
    tunnel.write_all(&expected).await.unwrap();
    tunnel.shutdown().await.unwrap();
    let mut received = Vec::new();
    tunnel.read_to_end(&mut received).await.unwrap();
    assert_eq!(received, expected);
    finished_rx.await.unwrap();
    drop(tunnel);
    drop(response);
    drop(sender);
    connection.await.unwrap().unwrap();
    server.await.unwrap().unwrap();
}
