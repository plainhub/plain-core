use super::Bits;
use crate::anyhow::{Result, bail};

pub(super) struct Codebook {
    pub(super) counts: [u8; 16],
    pub(super) values: Vec<u8>,
    minimum: [u32; 17],
    maximum: [i32; 17],
    offset: [usize; 17],
}

impl Codebook {
    pub(super) fn new(counts: &[u8; 16], values: &[u8]) -> Result<Self> {
        let symbols = counts.iter().map(|&v| usize::from(v)).sum::<usize>();
        if symbols == 0 || symbols > 256 || symbols != values.len() {
            bail!("invalid Huffman symbol count");
        }
        let mut minimum = [0; 17];
        let mut maximum = [-1; 17];
        let mut offset = [0; 17];
        let mut code = 0u32;
        let mut position = 0usize;
        for length in 1..=16 {
            let count = u32::from(counts[length - 1]);
            if code + count > 1 << length {
                bail!("oversubscribed Huffman table");
            }
            minimum[length] = code;
            maximum[length] = (code + count) as i32 - 1;
            offset[length] = position;
            code = (code + count) << 1;
            position += count as usize;
        }
        Ok(Self {
            counts: *counts,
            values: values.to_vec(),
            minimum,
            maximum,
            offset,
        })
    }

    pub(super) fn entries(&self) -> impl Iterator<Item = (u16, u8, u8)> + '_ {
        (1..=16).flat_map(move |length| {
            (0..usize::from(self.counts[length - 1])).map(move |i| {
                (
                    (self.minimum[length] + i as u32) as u16,
                    length as u8,
                    self.values[self.offset[length] + i],
                )
            })
        })
    }

    pub(super) fn symbol(&self, bits: &mut Bits) -> Result<u8> {
        let mut code = 0i32;
        for length in 1..=16 {
            code = (code << 1)
                | bits
                    .bit()
                    .ok_or_else(|| crate::anyhow::anyhow!("truncated Huffman symbol"))?
                    as i32;
            if code >= self.minimum[length] as i32 && code <= self.maximum[length] {
                return Ok(self.values
                    [self.offset[length] + (code - self.minimum[length] as i32) as usize]);
            }
        }
        bail!("invalid Huffman symbol")
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/pjpeg/huffman.rs"]
mod tests;
