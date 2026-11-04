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

use super::{
    endian::{BigEndian, Endian, LittleEndian},
    error::Error,
};
use std::collections::HashSet;

const TIFF_BE: u16 = 0x4d4d;
const TIFF_LE: u16 = 0x4949;
const TIFF_FORTY_TWO: u16 = 42;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Context {
    Tiff,
    Exif,
    Gps,
    Interop,
}

struct Entry {
    kind: u16,
    count: u32,
    offset: usize,
}

#[derive(Default)]
struct Parser {
    validated_children: HashSet<(usize, Context)>,
    remaining_entries: usize,
    #[cfg(test)]
    ifd_reads: usize,
}

pub fn validate(data: &[u8]) -> Result<(), Error> {
    Parser::default().parse(data)
}

impl Parser {
    pub fn parse(&mut self, data: &[u8]) -> Result<(), Error> {
        self.remaining_entries = data.len() / 2;
        // Check the byte order and call the real parser.
        if data.len() < 8 {
            return Err(Error::InvalidFormat("Truncated TIFF header"));
        }
        match BigEndian::loadu16(data, 0) {
            TIFF_BE => self.parse_header::<BigEndian>(data),
            TIFF_LE => self.parse_header::<LittleEndian>(data),
            _ => Err(Error::InvalidFormat("Invalid TIFF byte order")),
        }
    }

    fn parse_header<E>(&mut self, data: &[u8]) -> Result<(), Error>
    where
        E: Endian,
    {
        // Parse the rest of the header (42 and the IFD offset).
        if E::loadu16(data, 2) != TIFF_FORTY_TWO {
            return Err(Error::InvalidFormat("Invalid forty two"));
        }
        let ifd_offset = E::loadu32(data, 4) as usize;
        self.parse_body::<E>(data, ifd_offset)
    }

    fn parse_body<E>(&mut self, data: &[u8], mut ifd_offset: usize) -> Result<(), Error>
    where
        E: Endian,
    {
        let mut ifd_count = 0;
        while ifd_offset != 0 {
            // Limit the number of IFDs to defend against resource exhaustion
            // attacks.
            if ifd_count >= 8 {
                return Err(Error::InvalidFormat("Limit the IFD count to 8"));
            }
            ifd_offset = self.parse_ifd::<E>(data, ifd_offset, Context::Tiff)?;
            ifd_count += 1;
        }
        Ok(())
    }

    // Parse IFD [EXIF23 4.6.2].
    fn parse_ifd<E>(&mut self, data: &[u8], mut offset: usize, ctx: Context) -> Result<usize, Error>
    where
        E: Endian,
    {
        #[cfg(test)]
        {
            self.ifd_reads += 1;
        }
        // Count (the number of the entries).
        if data.len() < offset || data.len() - offset < 2 {
            return Err(Error::InvalidFormat("Truncated IFD count"));
        }
        let count = E::loadu16(data, offset) as usize;
        offset += 2;
        self.remaining_entries = self
            .remaining_entries
            .checked_sub(count)
            .ok_or(Error::InvalidFormat("Exif directory work limit exceeded"))?;

        // Array of entries.
        for _ in 0..count {
            if data.len() - offset < 12 {
                return Err(Error::InvalidFormat("Truncated IFD"));
            }
            let (tag, val) = Self::parse_ifd_entry::<E>(data, offset)?;
            offset += 12;

            // No infinite recursion will occur because the context is not
            // recursively defined.
            let tag = (ctx, tag);
            let child_ctx = match tag {
                (Context::Tiff, 0x8769) => Context::Exif,
                (Context::Tiff, 0x8825) => Context::Gps,
                (Context::Exif, 0xa005) => Context::Interop,
                _ => {
                    continue;
                }
            };
            self.parse_child_ifd::<E>(data, val, child_ctx)?;
        }

        // Offset to the next IFD.
        if data.len() - offset < 4 {
            return Err(Error::InvalidFormat("Truncated next IFD offset"));
        }
        let next_ifd_offset = E::loadu32(data, offset);
        Ok(next_ifd_offset as usize)
    }

    fn parse_ifd_entry<E>(data: &[u8], offset: usize) -> Result<(u16, Entry), Error>
    where
        E: Endian,
    {
        // The size of entry has been checked in parse_ifd().
        let tag = E::loadu16(data, offset);
        let typ = E::loadu16(data, offset + 2);
        let cnt = E::loadu32(data, offset + 4);
        let valofs_at = offset + 8;
        let unitlen: usize = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => 0,
        };
        let vallen = unitlen
            .checked_mul(cnt as usize)
            .ok_or(Error::InvalidFormat("Invalid entry count"))?;
        let val = if vallen <= 4 {
            Entry {
                kind: typ,
                count: cnt,
                offset: valofs_at,
            }
        } else {
            let ofs = E::loadu32(data, valofs_at) as usize;
            if data.len() < ofs || data.len() - ofs < vallen {
                return Err(Error::InvalidFormat("Truncated field value"));
            }
            Entry {
                kind: typ,
                count: cnt,
                offset: ofs,
            }
        };
        Ok((tag, val))
    }

    fn parse_child_ifd<E>(&mut self, data: &[u8], pointer: Entry, ctx: Context) -> Result<(), Error>
    where
        E: Endian,
    {
        let ofs = match (pointer.kind, pointer.count) {
            (_, 0) => return Err(Error::InvalidFormat("Invalid pointer")),
            (1, _) => u32::from(data[pointer.offset]),
            (3, _) => u32::from(E::loadu16(data, pointer.offset)),
            (4, _) => E::loadu32(data, pointer.offset),
            _ => return Err(Error::InvalidFormat("Invalid pointer")),
        } as usize;
        if self.validated_children.contains(&(ofs, ctx)) {
            return Ok(());
        }
        match self.parse_ifd::<E>(data, ofs, ctx)? {
            0 => {
                self.validated_children.insert((ofs, ctx));
                Ok(())
            }
            _ => Err(Error::InvalidFormat("Unexpected next IFD")),
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/utils/image_exif/container/tiff.rs"]
mod tests;
