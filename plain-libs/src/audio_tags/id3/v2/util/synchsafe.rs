use crate::audio_tags::error::Result;

use std::io::Read;

pub struct UnsynchronizedStream<R> {
    reader: R,
    // Same buffer size as `BufReader`
    buf: [u8; 8 * 1024],
    bytes_available: usize,
    pos: usize,
    encountered_ff: bool,
}

impl<R> UnsynchronizedStream<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buf: [0; 8 * 1024],
            bytes_available: 0,
            pos: 0,
            encountered_ff: false,
        }
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    pub fn get_ref(&self) -> &R {
        &self.reader
    }
}

impl<R: Read> Read for UnsynchronizedStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let dest_len = buf.len();
        if dest_len == 0 {
            return Ok(0);
        }

        let mut dest_pos = 0;
        loop {
            if dest_pos == dest_len {
                break;
            }

            if self.pos >= self.bytes_available {
                self.bytes_available = self.reader.read(&mut self.buf)?;
                self.pos = 0;
            }

            // Exhausted the reader
            if self.bytes_available == 0 {
                break;
            }

            if self.encountered_ff {
                self.encountered_ff = false;

                // Only skip the next byte if this is valid unsynchronization
                // Otherwise just continue as normal
                if self.buf[self.pos] == 0 {
                    self.pos += 1;
                    continue;
                }
            }

            let current_byte = self.buf[self.pos];
            buf[dest_pos] = current_byte;
            dest_pos += 1;
            self.pos += 1;

            if current_byte == 0xFF {
                self.encountered_ff = true;
            }
        }

        Ok(dest_pos)
    }
}

pub trait SynchsafeInteger: Sized {
    type WideningType;

    fn synch(self) -> Result<Self>;

    fn widening_synch(self) -> Self::WideningType;

    fn unsynch(self) -> Self;
}

macro_rules! impl_synchsafe {
    (
		$ty:ty, $widening_ty:ty,
		synch($n:ident) $body:block;
		widening_synch($w:ident) $widening_body:block;
		unsynch($u:ident) $unsynch_body:block
	) => {
        impl SynchsafeInteger for $ty {
            type WideningType = $widening_ty;

            fn synch(self) -> Result<Self> {
                const MAXIMUM_INTEGER: $ty = {
                    let num_bytes = core::mem::size_of::<$ty>();
                    // 7 bits are available per byte, shave off 1 bit per byte
                    <$ty>::MAX >> num_bytes
                };

                if self > MAXIMUM_INTEGER {
                    crate::audio_tags::macros::err!(TooMuchData);
                }

                let $n = self;
                Ok($body)
            }

            fn widening_synch(self) -> Self::WideningType {
                let mut $w = <$widening_ty>::MIN;
                let $n = self;
                $widening_body;
                $w
            }

            fn unsynch(self) -> Self {
                let $u = self;
                $unsynch_body
            }
        }
    };
}

impl_synchsafe! {
    u8, u16,
    synch(n) {
        (n & 0x7F)
    };
    widening_synch(w) {
        w |= u16::from(n & 0x7F);
        w |= u16::from(n & 0x80) << 1;
    };
    unsynch(u) {
        (u & 0x7F)
    }
}

impl_synchsafe! {
    u16, u32,
    synch(n) {
        (n & 0x7F) |
        ((n & (0x7F << 7)) << 1)
    };
    widening_synch(w) {
        w |= u32::from(n & 0x7F);
        w |= u32::from((n & (0x7F << 7)) << 1);
        w |= u32::from(n & (0x03 << 14)) << 2;
    };
    unsynch(u) {
        ((u & 0x7F00) >> 1) | (u & 0x7F)
    }
}

impl_synchsafe! {
    u32, u64,
    synch(n) {
        (n & 0x7F) |
        ((n & (0x7F << 7)) << 1) |
        ((n & (0x7F << 14)) << 2) |
        ((n & (0x7F << 21)) << 3)
    };
    widening_synch(w) {
        w |= u64::from(n & 0x7F);
        w |= u64::from(n & (0x7F << 7)) << 1;
        w |= u64::from(n & (0x7F << 14)) << 2;
        w |= u64::from(n & (0x7F << 21)) << 3;
        w |= u64::from(n & (0x0F << 28)) << 4;
    };
    unsynch(u) {
        ((u & 0x7F00_0000) >> 3) | ((u & 0x7F_0000) >> 2) | ((u & 0x7F00) >> 1) | (u & 0x7F)
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/util/synchsafe/tests.rs"]
mod tests;
