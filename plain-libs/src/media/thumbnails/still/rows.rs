use super::super::scale::BoxRowFilter;

pub(super) struct Rows<const CHANNELS: usize> {
    width: u32,
    height: u32,
    factor: u32,
    seen: u32,
    buffered: u32,
    vertical: Vec<u32>,
    filter: BoxRowFilter,
    output: Vec<u8>,
}

impl<const CHANNELS: usize> Rows<CHANNELS> {
    pub(super) fn new(width: u32, height: u32, tw: u32, th: u32) -> Self {
        let factor = (width / tw).min(height / th).clamp(1, 64);
        Self {
            width,
            height,
            factor,
            seen: 0,
            buffered: 0,
            vertical: if factor == 1 {
                Vec::new()
            } else {
                vec![0; width as usize * CHANNELS]
            },
            filter: BoxRowFilter::new(width, factor),
            output: Vec::with_capacity(
                width.div_ceil(factor) as usize * height.div_ceil(factor) as usize * CHANNELS,
            ),
        }
    }
    pub(super) fn push(&mut self, row: &[u8]) {
        if self.factor == 1 {
            self.output.extend_from_slice(row);
            return;
        }
        if self.buffered == 0 {
            for (sum, value) in self.vertical.iter_mut().zip(row) {
                *sum = u32::from(*value);
            }
        } else {
            for (sum, value) in self.vertical.iter_mut().zip(row) {
                *sum += u32::from(*value);
            }
        }
        self.seen += 1;
        self.buffered += 1;
        if self.buffered == self.factor || self.seen == self.height {
            self.filter
                .append::<CHANNELS>(&mut self.vertical, self.buffered, &mut self.output);
            self.buffered = 0;
        }
    }
    pub(super) fn finish(self) -> (Vec<u8>, u32, u32) {
        (
            self.output,
            self.width.div_ceil(self.factor),
            self.height.div_ceil(self.factor),
        )
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/still/rows.rs"]
mod tests;
