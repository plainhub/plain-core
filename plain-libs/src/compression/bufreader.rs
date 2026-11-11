use crate::compression::io;
use crate::compression::io::{BufRead, Read};
use core::cmp;
use core::mem;
use std::boxed::Box;
use std::vec::Vec;
pub struct BufReader<R> {
    inner: R,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
}
impl<R> ::core::fmt::Debug for BufReader<R>
where
    R: ::core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error> {
        fmt.debug_struct("BufReader")
            .field("reader", &self.inner)
            .field(
                "buffer",
                &format_args!("{}/{}", self.cap - self.pos, self.buf.len()),
            )
            .finish()
    }
}
impl<R: Read> BufReader<R> {
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_buf(vec![0; 32 * 1024], inner)
    }
    pub fn with_buf(buf: Vec<u8>, inner: R) -> BufReader<R> {
        BufReader {
            inner,
            buf: buf.into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }
}
impl<R> BufReader<R> {
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
    pub fn reset(&mut self, inner: R) -> R {
        self.pos = 0;
        self.cap = 0;
        mem::replace(&mut self.inner, inner)
    }
}
impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos == self.cap && buf.len() >= self.buf.len() {
            return self.inner.read(buf);
        }
        let nread = {
            let mut rem = self.fill_buf()?;
            rem.read(buf)?
        };
        self.consume(nread);
        Ok(nread)
    }
}
impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.pos == self.cap {
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }
    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.cap);
    }
}
