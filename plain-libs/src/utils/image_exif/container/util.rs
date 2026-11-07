//
// Copyright (c) 2016 KAMADA Ken'ichi.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
// 1. Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in the
//    documentation and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
// OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
// HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
// LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
// OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
// SUCH DAMAGE.
//

use std::io;
use std::io::Read as _;

pub trait BufReadExt {
    fn discard_exact(&mut self, len: usize) -> io::Result<()>;
    fn is_eof(&mut self) -> io::Result<bool>;
}

impl<T> BufReadExt for T
where
    T: io::BufRead,
{
    fn discard_exact(&mut self, mut len: usize) -> io::Result<()> {
        while len > 0 {
            let consume_len = match self.fill_buf() {
                Ok(buf) if buf.is_empty() => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "unexpected EOF",
                    ));
                }
                Ok(buf) => buf.len().min(len),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            self.consume(consume_len);
            len -= consume_len;
        }
        Ok(())
    }

    fn is_eof(&mut self) -> io::Result<bool> {
        loop {
            match self.fill_buf() {
                Ok(buf) => return Ok(buf.is_empty()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }
}

pub trait ReadExt {
    fn read_exact_len(&mut self, buf: &mut Vec<u8>, len: usize) -> io::Result<()>;
}

impl<T> ReadExt for T
where
    T: io::Read,
{
    fn read_exact_len(&mut self, buf: &mut Vec<u8>, len: usize) -> io::Result<()> {
        if len > super::MAX_METADATA_BYTES.saturating_sub(buf.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Exif metadata too large",
            ));
        }
        // Using `vec![0; len]` and `read_exact` is more efficient but
        // less robust against broken files; a small file can easily
        // trigger OOM by a huge length value without actual data.
        // When the fallible allocation feature is stabilized,
        // we could revisit this.
        if self.take(len as u64).read_to_end(buf)? != len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unexpected EOF",
            ));
        }
        Ok(())
    }
}

pub fn read_to_end(reader: &mut impl io::Read, buf: &mut Vec<u8>, limit: usize) -> io::Result<()> {
    let remaining = limit.saturating_sub(buf.len());
    reader.take(remaining as u64 + 1).read_to_end(buf)?;
    if buf.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Exif metadata too large",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/utils/image_exif/container/util.rs"]
mod tests;
