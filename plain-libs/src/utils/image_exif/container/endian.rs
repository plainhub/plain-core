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

use std::mem;

// This is a module to select endianess by using generics
// in order to avoid run-time dispatching penalty at the cost of
// increased object size.

pub trait Endian {
    fn loadu16(buf: &[u8], from: usize) -> u16;
    fn loadu32(buf: &[u8], from: usize) -> u32;
    fn loadu64(buf: &[u8], from: usize) -> u64;
}

pub struct BigEndian;
pub struct LittleEndian;

macro_rules! generate_load {
    ($name:ident, $int_type:ident, $from_func:ident) => {
        fn $name(buf: &[u8], offset: usize) -> $int_type {
            let mut num = [0u8; mem::size_of::<$int_type>()];
            num.copy_from_slice(&buf[offset..offset + mem::size_of::<$int_type>()]);
            $int_type::$from_func(num)
        }
    };
}

impl Endian for BigEndian {
    generate_load!(loadu16, u16, from_be_bytes);
    generate_load!(loadu32, u32, from_be_bytes);
    generate_load!(loadu64, u64, from_be_bytes);
}

impl Endian for LittleEndian {
    generate_load!(loadu16, u16, from_le_bytes);
    generate_load!(loadu32, u32, from_le_bytes);
    generate_load!(loadu64, u64, from_le_bytes);
}
