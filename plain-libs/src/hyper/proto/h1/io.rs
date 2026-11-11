use super::{Http1Transaction, ParseContext, ParsedMessage};
use crate::hyper::common::buf::BufList;
use crate::hyper::rt::{Read, ReadBuf, Write};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_core::ready;
use std::cmp;
use std::fmt;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};
pub(crate) const INIT_BUFFER_SIZE: usize = 8192;
pub(crate) const MINIMUM_MAX_BUFFER_SIZE: usize = INIT_BUFFER_SIZE;
pub(crate) const DEFAULT_MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;
const MAX_BUF_LIST_BUFFERS: usize = 16;
pub(crate) struct Buffered<T, B> {
    flush_pipeline: bool,
    io: T,
    partial_len: Option<usize>,
    read_blocked: bool,
    read_buf: BytesMut,
    read_buf_strategy: ReadStrategy,
    write_buf: WriteBuf<B>,
}
impl<T, B> fmt::Debug for Buffered<T, B>
where
    B: Buf,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("read_buf", &self.read_buf)
            .field("write_buf", &self.write_buf)
            .finish()
    }
}
impl<T, B> Buffered<T, B>
where
    T: Read + Write + Unpin,
    B: Buf,
{
    pub(crate) fn new(io: T) -> Buffered<T, B> {
        let strategy = if io.is_write_vectored() {
            WriteStrategy::Queue
        } else {
            WriteStrategy::Flatten
        };
        let write_buf = WriteBuf::new(strategy);
        Buffered {
            flush_pipeline: false,
            io,
            partial_len: None,
            read_blocked: false,
            read_buf: BytesMut::with_capacity(0),
            read_buf_strategy: ReadStrategy::default(),
            write_buf,
        }
    }
    pub(crate) fn set_flush_pipeline(&mut self, enabled: bool) {
        debug_assert!(!self.write_buf.has_remaining());
        self.flush_pipeline = enabled;
        if enabled {
            self.set_write_strategy_flatten();
        }
    }
    pub(crate) fn set_max_buf_size(&mut self, max: usize) {
        assert!(
            max >= MINIMUM_MAX_BUFFER_SIZE,
            "The max_buf_size cannot be smaller than {MINIMUM_MAX_BUFFER_SIZE}.",
        );
        self.read_buf_strategy = ReadStrategy::with_max(max);
        self.write_buf.max_buf_size = max;
    }
    pub(crate) fn set_read_buf_exact_size(&mut self, sz: usize) {
        self.read_buf_strategy = ReadStrategy::Exact(sz);
    }
    pub(crate) fn set_write_strategy_flatten(&mut self) {
        debug_assert_eq!(self.write_buf.queue.bufs_cnt(), 0);
        self.write_buf.set_strategy(WriteStrategy::Flatten);
    }
    pub(crate) fn set_write_strategy_queue(&mut self) {
        debug_assert_eq!(self.write_buf.queue.bufs_cnt(), 0);
        self.write_buf.set_strategy(WriteStrategy::Queue);
    }
    pub(crate) fn read_buf(&self) -> &[u8] {
        self.read_buf.as_ref()
    }
    fn read_buf_remaining_mut(&self) -> usize {
        self.read_buf.capacity() - self.read_buf.len()
    }
    pub(crate) fn can_headers_buf(&self) -> bool {
        !self.write_buf.queue.has_remaining()
    }
    pub(crate) fn headers_buf(&mut self) -> &mut Vec<u8> {
        let buf = self.write_buf.headers_mut();
        &mut buf.bytes
    }
    pub(super) fn write_buf(&mut self) -> &mut WriteBuf<B> {
        &mut self.write_buf
    }
    pub(crate) fn buffer<BB: Buf + Into<B>>(&mut self, buf: BB) {
        self.write_buf.buffer(buf);
    }
    pub(crate) fn has_buffered_write(&self) -> bool {
        self.write_buf.remaining() > 0
    }
    pub(crate) fn can_buffer(&self) -> bool {
        self.flush_pipeline || self.write_buf.can_buffer()
    }
    pub(crate) fn consume_leading_lines(&mut self) {
        if !self.read_buf.is_empty() {
            let mut i = 0;
            while i < self.read_buf.len() {
                match self.read_buf[i] {
                    b'\r' | b'\n' => i += 1,
                    _ => break,
                }
            }
            self.read_buf.advance(i);
        }
    }
    pub(super) fn parse<S>(
        &mut self,
        cx: &mut Context<'_>,
        parse_ctx: ParseContext<'_>,
    ) -> Poll<crate::hyper::Result<ParsedMessage<S::Incoming>>>
    where
        S: Http1Transaction,
    {
        loop {
            if let Some(msg) = super::role::parse_headers::<S>(
                &mut self.read_buf,
                self.partial_len,
                ParseContext {
                    cached_headers: parse_ctx.cached_headers,
                    req_method: parse_ctx.req_method,
                    h1_parser_config: parse_ctx.h1_parser_config.clone(),
                    h1_max_headers: parse_ctx.h1_max_headers,
                    preserve_header_case: parse_ctx.preserve_header_case,
                    h09_responses: parse_ctx.h09_responses,
                    on_informational: parse_ctx.on_informational,
                },
            )? {
                debug!("parsed {} headers", msg.head.headers.len());
                self.partial_len = None;
                return Poll::Ready(Ok(msg));
            } else {
                let max = self.read_buf_strategy.max();
                let curr_len = self.read_buf.len();
                if curr_len >= max {
                    debug!("max_buf_size ({}) reached, closing", max);
                    return Poll::Ready(Err(crate::hyper::Error::new_too_large()));
                }
                if curr_len > 0 {
                    trace!("partial headers; {} bytes so far", curr_len);
                    self.partial_len = Some(curr_len);
                } else {
                    self.partial_len = None;
                }
            }
            if ready!(self.poll_read_from_io(cx)).map_err(crate::hyper::Error::new_io)? == 0 {
                trace!("parse eof");
                return Poll::Ready(Err(crate::hyper::Error::new_incomplete()));
            }
        }
    }
    pub(crate) fn poll_read_from_io(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.read_blocked = false;
        let next = cmp::min(
            self.read_buf_strategy.next(),
            self.read_buf_strategy
                .max()
                .saturating_sub(self.read_buf.len()),
        );
        if self.read_buf_remaining_mut() < next {
            self.read_buf.reserve(next);
        }
        let dst = unsafe { self.read_buf.chunk_mut().as_uninit_slice_mut() };
        let mut buf = ReadBuf::uninit(dst);
        match Pin::new(&mut self.io).poll_read(cx, buf.unfilled()) {
            Poll::Ready(Ok(_)) => {
                let n = buf.filled().len();
                trace!("received {} bytes", n);
                unsafe {
                    self.read_buf.advance_mut(n);
                }
                self.read_buf_strategy.record(n);
                Poll::Ready(Ok(n))
            }
            Poll::Pending => {
                self.read_blocked = true;
                Poll::Pending
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
    pub(crate) fn into_inner(self) -> (T, Bytes) {
        (self.io, self.read_buf.freeze())
    }
    pub(crate) fn is_read_blocked(&self) -> bool {
        self.read_blocked
    }
    pub(crate) fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.flush_pipeline && !self.read_buf.is_empty() {
            Poll::Ready(Ok(()))
        } else if self.write_buf.remaining() == 0 {
            Pin::new(&mut self.io).poll_flush(cx)
        } else {
            if let WriteStrategy::Flatten = self.write_buf.strategy {
                return self.poll_flush_flattened(cx);
            }
            const MAX_WRITEV_BUFS: usize = 64;
            loop {
                let n = {
                    let mut iovs = [IoSlice::new(&[]); MAX_WRITEV_BUFS];
                    let len = self.write_buf.chunks_vectored(&mut iovs);
                    ready!(Pin::new(&mut self.io).poll_write_vectored(cx, &iovs[..len]))?
                };
                self.write_buf.advance(n);
                debug!("flushed {} bytes", n);
                if self.write_buf.remaining() == 0 {
                    break;
                } else if n == 0 {
                    trace!(
                        "write returned zero, but {} bytes remaining",
                        self.write_buf.remaining()
                    );
                    return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                }
            }
            Pin::new(&mut self.io).poll_flush(cx)
        }
    }
    fn poll_flush_flattened(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let n = ready!(Pin::new(&mut self.io).poll_write(cx, self.write_buf.headers.chunk()))?;
            debug!("flushed {} bytes", n);
            self.write_buf.headers.advance(n);
            if self.write_buf.headers.remaining() == 0 {
                self.write_buf.headers.reset();
                break;
            } else if n == 0 {
                trace!(
                    "write returned zero, but {} bytes remaining",
                    self.write_buf.remaining()
                );
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }
        Pin::new(&mut self.io).poll_flush(cx)
    }
    pub(crate) fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.poll_flush(cx))?;
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}
impl<T: Unpin, B> Unpin for Buffered<T, B> {}
pub(crate) trait MemRead {
    fn read_mem(&mut self, cx: &mut Context<'_>, len: usize) -> Poll<io::Result<Bytes>>;
}
impl<T, B> MemRead for Buffered<T, B>
where
    T: Read + Write + Unpin,
    B: Buf,
{
    fn read_mem(&mut self, cx: &mut Context<'_>, len: usize) -> Poll<io::Result<Bytes>> {
        if !self.read_buf.is_empty() {
            let n = std::cmp::min(len, self.read_buf.len());
            Poll::Ready(Ok(self.read_buf.split_to(n).freeze()))
        } else {
            let n = ready!(self.poll_read_from_io(cx))?;
            Poll::Ready(Ok(self.read_buf.split_to(::std::cmp::min(len, n)).freeze()))
        }
    }
}
#[derive(Clone, Copy, Debug)]
enum ReadStrategy {
    Adaptive {
        decrease_now: bool,
        next: usize,
        max: usize,
    },
    Exact(usize),
}
impl ReadStrategy {
    fn with_max(max: usize) -> ReadStrategy {
        ReadStrategy::Adaptive {
            decrease_now: false,
            next: INIT_BUFFER_SIZE,
            max,
        }
    }
    fn next(&self) -> usize {
        match *self {
            ReadStrategy::Adaptive { next, .. } => next,
            ReadStrategy::Exact(exact) => exact,
        }
    }
    fn max(&self) -> usize {
        match *self {
            ReadStrategy::Adaptive { max, .. } => max,
            ReadStrategy::Exact(exact) => exact,
        }
    }
    fn record(&mut self, bytes_read: usize) {
        match self {
            ReadStrategy::Adaptive {
                decrease_now,
                next,
                max,
                ..
            } => {
                if bytes_read >= *next {
                    *next = cmp::min(incr_power_of_two(*next), *max);
                    *decrease_now = false;
                } else {
                    let decr_to = prev_power_of_two(*next);
                    if bytes_read < decr_to {
                        if *decrease_now {
                            *next = cmp::max(decr_to, INIT_BUFFER_SIZE);
                            *decrease_now = false;
                        } else {
                            *decrease_now = true;
                        }
                    } else {
                        *decrease_now = false;
                    }
                }
            }
            ReadStrategy::Exact(_) => (),
        }
    }
}
fn incr_power_of_two(n: usize) -> usize {
    n.saturating_mul(2)
}
fn prev_power_of_two(n: usize) -> usize {
    debug_assert!(n >= 4);
    (usize::MAX >> (n.leading_zeros() + 2)) + 1
}
impl Default for ReadStrategy {
    fn default() -> ReadStrategy {
        ReadStrategy::with_max(DEFAULT_MAX_BUFFER_SIZE)
    }
}
#[derive(Clone)]
pub(crate) struct Cursor<T> {
    bytes: T,
    pos: usize,
}
impl<T: AsRef<[u8]>> Cursor<T> {
    #[inline]
    pub(crate) fn new(bytes: T) -> Cursor<T> {
        Cursor { bytes, pos: 0 }
    }
}
impl Cursor<Vec<u8>> {
    fn maybe_unshift(&mut self, additional: usize) {
        if self.pos == 0 {
            return;
        }
        if self.bytes.capacity() - self.bytes.len() >= additional {
            return;
        }
        self.bytes.drain(0..self.pos);
        self.pos = 0;
    }
    fn reset(&mut self) {
        self.pos = 0;
        self.bytes.clear();
    }
}
impl<T: AsRef<[u8]>> fmt::Debug for Cursor<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cursor")
            .field("pos", &self.pos)
            .field("len", &self.bytes.as_ref().len())
            .finish()
    }
}
impl<T: AsRef<[u8]>> Buf for Cursor<T> {
    #[inline]
    fn remaining(&self) -> usize {
        self.bytes.as_ref().len() - self.pos
    }
    #[inline]
    fn chunk(&self) -> &[u8] {
        &self.bytes.as_ref()[self.pos..]
    }
    #[inline]
    fn advance(&mut self, cnt: usize) {
        debug_assert!(self.pos + cnt <= self.bytes.as_ref().len());
        self.pos += cnt;
    }
}
pub(super) struct WriteBuf<B> {
    headers: Cursor<Vec<u8>>,
    max_buf_size: usize,
    queue: BufList<B>,
    strategy: WriteStrategy,
}
impl<B: Buf> WriteBuf<B> {
    fn new(strategy: WriteStrategy) -> WriteBuf<B> {
        WriteBuf {
            headers: Cursor::new(Vec::with_capacity(INIT_BUFFER_SIZE)),
            max_buf_size: DEFAULT_MAX_BUFFER_SIZE,
            queue: BufList::new(),
            strategy,
        }
    }
}
impl<B> WriteBuf<B>
where
    B: Buf,
{
    fn set_strategy(&mut self, strategy: WriteStrategy) {
        self.strategy = strategy;
    }
    pub(super) fn buffer<BB: Buf + Into<B>>(&mut self, mut buf: BB) {
        debug_assert!(buf.has_remaining());
        match self.strategy {
            WriteStrategy::Flatten => {
                let head = self.headers_mut();
                head.maybe_unshift(buf.remaining());
                trace!(
                    self.len = head.remaining(),
                    buf.len = buf.remaining(),
                    "buffer.flatten"
                );
                loop {
                    let adv = {
                        let slice = buf.chunk();
                        if slice.is_empty() {
                            return;
                        }
                        head.bytes.extend_from_slice(slice);
                        slice.len()
                    };
                    buf.advance(adv);
                }
            }
            WriteStrategy::Queue => {
                trace!(
                    self.len = self.remaining(),
                    buf.len = buf.remaining(),
                    "buffer.queue"
                );
                self.queue.push(buf.into());
            }
        }
    }
    fn can_buffer(&self) -> bool {
        match self.strategy {
            WriteStrategy::Flatten => self.remaining() < self.max_buf_size,
            WriteStrategy::Queue => {
                self.queue.bufs_cnt() < MAX_BUF_LIST_BUFFERS && self.remaining() < self.max_buf_size
            }
        }
    }
    fn headers_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        debug_assert!(!self.queue.has_remaining());
        &mut self.headers
    }
}
impl<B: Buf> fmt::Debug for WriteBuf<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteBuf")
            .field("remaining", &self.remaining())
            .field("strategy", &self.strategy)
            .finish()
    }
}
impl<B: Buf> Buf for WriteBuf<B> {
    #[inline]
    fn remaining(&self) -> usize {
        self.headers.remaining() + self.queue.remaining()
    }
    #[inline]
    fn chunk(&self) -> &[u8] {
        let headers = self.headers.chunk();
        if !headers.is_empty() {
            headers
        } else {
            self.queue.chunk()
        }
    }
    #[inline]
    fn advance(&mut self, cnt: usize) {
        let hrem = self.headers.remaining();
        match hrem.cmp(&cnt) {
            cmp::Ordering::Equal => self.headers.reset(),
            cmp::Ordering::Greater => self.headers.advance(cnt),
            cmp::Ordering::Less => {
                let qcnt = cnt - hrem;
                self.headers.reset();
                self.queue.advance(qcnt);
            }
        }
    }
    #[inline]
    fn chunks_vectored<'t>(&'t self, dst: &mut [IoSlice<'t>]) -> usize {
        let n = self.headers.chunks_vectored(dst);
        self.queue.chunks_vectored(&mut dst[n..]) + n
    }
}
#[derive(Debug)]
enum WriteStrategy {
    Flatten,
    Queue,
}
