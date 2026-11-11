use crate::compression::io;
use crate::compression::io::{BufRead, Write};
use crate::compression::{
    Compress, CompressError, Decompress, DecompressError, FlushCompress, FlushDecompress, Status,
};
use core::mem;
use std::vec::Vec;
#[derive(Debug)]
pub struct Writer<W: Write, D: Ops> {
    obj: Option<W>,
    pub data: D,
    buf: Vec<u8>,
}
pub trait Ops {
    type Error: Into<io::Error>;
    type Flush: Flush;
    fn total_in(&self) -> u64;
    fn total_out(&self) -> u64;
    fn run(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: Self::Flush,
    ) -> Result<Status, Self::Error>;
    fn run_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: Self::Flush,
    ) -> Result<Status, Self::Error>;
}
impl Ops for Compress {
    type Error = CompressError;
    type Flush = FlushCompress;
    fn total_in(&self) -> u64 {
        self.total_in()
    }
    fn total_out(&self) -> u64 {
        self.total_out()
    }
    fn run(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.compress(input, output, flush)
    }
    fn run_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.compress_vec(input, output, flush)
    }
}
impl Ops for Decompress {
    type Error = DecompressError;
    type Flush = FlushDecompress;
    fn total_in(&self) -> u64 {
        self.total_in()
    }
    fn total_out(&self) -> u64 {
        self.total_out()
    }
    fn run(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.decompress(input, output, flush)
    }
    fn run_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.decompress_vec(input, output, flush)
    }
}
pub trait Flush {
    fn none() -> Self;
    fn sync() -> Self;
    fn finish() -> Self;
}
impl Flush for FlushCompress {
    fn none() -> Self {
        FlushCompress::None
    }
    fn sync() -> Self {
        FlushCompress::Sync
    }
    fn finish() -> Self {
        FlushCompress::Finish
    }
}
impl Flush for FlushDecompress {
    fn none() -> Self {
        FlushDecompress::None
    }
    fn sync() -> Self {
        FlushDecompress::Sync
    }
    fn finish() -> Self {
        FlushDecompress::Finish
    }
}
pub fn read<R, D>(obj: &mut R, data: &mut D, dst: &mut [u8]) -> io::Result<usize>
where
    R: BufRead,
    D: Ops,
{
    loop {
        let (read, consumed, ret, eof);
        {
            let input = obj.fill_buf()?;
            eof = input.is_empty();
            let before_out = data.total_out();
            let before_in = data.total_in();
            let flush = if eof {
                D::Flush::finish()
            } else {
                D::Flush::none()
            };
            ret = data.run(input, dst, flush);
            read = (data.total_out() - before_out) as usize;
            consumed = (data.total_in() - before_in) as usize;
        }
        obj.consume(consumed);
        match ret {
            Ok(Status::Ok | Status::BufError) if read == 0 && !eof && !dst.is_empty() => continue,
            Ok(Status::Ok | Status::BufError) if read == 0 && eof && !dst.is_empty() => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete deflate stream",
                ));
            }
            Ok(Status::Ok | Status::BufError | Status::StreamEnd) => return Ok(read),
            Err(..) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "corrupt deflate stream",
                ));
            }
        }
    }
}
impl<W: Write, D: Ops> Writer<W, D> {
    pub fn new(w: W, d: D) -> Writer<W, D> {
        Writer {
            obj: Some(w),
            data: d,
            buf: Vec::with_capacity(32 * 1024),
        }
    }
    pub fn finish(&mut self) -> io::Result<()> {
        loop {
            self.dump()?;
            let before = self.data.total_out();
            self.data
                .run_vec(&[], &mut self.buf, Flush::finish())
                .map_err(Into::into)?;
            if before == self.data.total_out() {
                return Ok(());
            }
        }
    }
    pub fn replace(&mut self, w: W) -> W {
        self.buf.clear();
        mem::replace(self.get_mut(), w)
    }
    pub fn get_ref(&self) -> &W {
        self.obj.as_ref().unwrap()
    }
    pub fn get_mut(&mut self) -> &mut W {
        self.obj.as_mut().unwrap()
    }
    pub fn take_inner(&mut self) -> W {
        self.obj.take().unwrap()
    }

    pub(crate) fn write_with_status(&mut self, buf: &[u8]) -> io::Result<(usize, Status)> {
        loop {
            self.dump()?;
            let before_in = self.data.total_in();
            let ret = self.data.run_vec(buf, &mut self.buf, D::Flush::none());
            let written = (self.data.total_in() - before_in) as usize;
            let is_stream_end = matches!(ret, Ok(Status::StreamEnd));
            if !buf.is_empty() && written == 0 && ret.is_ok() && !is_stream_end {
                continue;
            }
            return match ret {
                Ok(st) => match st {
                    Status::Ok | Status::BufError | Status::StreamEnd => Ok((written, st)),
                },
                Err(..) => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "corrupt deflate stream",
                )),
            };
        }
    }
    fn dump(&mut self) -> io::Result<()> {
        while !self.buf.is_empty() {
            let n = self.obj.as_mut().unwrap().write(&self.buf)?;
            if n == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            self.buf.drain(..n);
        }
        Ok(())
    }
}
impl<W: Write, D: Ops> Write for Writer<W, D> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_with_status(buf).map(|res| res.0)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.data
            .run_vec(&[], &mut self.buf, Flush::sync())
            .map_err(Into::into)?;
        loop {
            self.dump()?;
            let before = self.data.total_out();
            self.data
                .run_vec(&[], &mut self.buf, Flush::none())
                .map_err(Into::into)?;
            if before == self.data.total_out() {
                break;
            }
        }
        self.obj.as_mut().unwrap().flush()
    }
}
impl<W: Write, D: Ops> Drop for Writer<W, D> {
    fn drop(&mut self) {
        if self.obj.is_some() {
            let _ = self.finish();
        }
    }
}
