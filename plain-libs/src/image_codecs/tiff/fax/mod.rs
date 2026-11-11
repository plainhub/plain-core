// Group 4 decoding adapted from fax 0.2.6 (SOURCE.json).
mod codes;
pub(super) mod decoder;

use std::ops::Not;

struct ByteReader<R> {
    read: R,
    partial: u32,
    valid: u8,
}
impl<E, R: Iterator<Item = Result<u8, E>>> ByteReader<R> {
    fn new(read: R) -> Result<Self, E> {
        let mut bits = ByteReader {
            read,
            partial: 0,
            valid: 0,
        };
        bits.fill()?;
        Ok(bits)
    }
    fn fill(&mut self) -> Result<(), E> {
        while self.valid < 16 {
            match self.read.next() {
                Some(Ok(byte)) => {
                    self.partial = self.partial << 8 | byte as u32;
                    self.valid += 8;
                }
                Some(Err(e)) => return Err(e),
                None => break,
            }
        }
        Ok(())
    }

    fn peek(&self, bits: u8) -> u16 {
        ((self.partial >> (self.valid - bits)) as u16) & ((1 << bits) - 1)
    }

    fn consume(&mut self, bits: u8) -> Result<(), E> {
        self.valid -= bits;
        self.fill()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}
impl Not for Color {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

struct Transitions<'a> {
    edges: &'a [u16],
    pos: usize,
}
impl<'a> Transitions<'a> {
    fn new(edges: &'a [u16]) -> Self {
        Transitions { edges, pos: 0 }
    }
    fn seek_back(&mut self, start: u16) {
        self.pos = self.pos.min(self.edges.len().saturating_sub(1));
        while self.pos > 0 {
            if start < self.edges[self.pos - 1] {
                self.pos -= 1;
            } else {
                break;
            }
        }
    }
    fn next_color(&mut self, start: u16, color: Color, start_of_row: bool) -> Option<u16> {
        if start_of_row {
            if color == Color::Black {
                self.pos = 1;
                return self.edges.get(0).cloned();
            } else {
                self.pos = 2;
                return self.edges.get(1).cloned();
            }
        }
        while self.pos < self.edges.len() {
            if self.edges[self.pos] <= start {
                self.pos += 1;
                continue;
            }

            if (self.pos % 2 == 0) != (color == Color::Black) {
                self.pos += 1;
            }

            break;
        }
        if self.pos < self.edges.len() {
            let val = self.edges[self.pos];
            self.pos += 1;
            Some(val)
        } else {
            None
        }
    }
    fn next(&mut self) -> Option<u16> {
        if self.pos < self.edges.len() {
            let val = self.edges[self.pos];
            self.pos += 1;
            Some(val)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/image_codecs/tiff/fax.rs"]
mod tests;
