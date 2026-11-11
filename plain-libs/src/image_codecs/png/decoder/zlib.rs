use super::{DecodingError, stream::FormatErrorInner, unfiltering_buffer::UnfilteringBuffer};
use crate::image_codecs::deflate::Decompressor;
const LOOKBACK_SIZE: usize = 32768;
pub struct UnfilterBuf<'data> {
    pub(crate) buffer: &'data mut [u8],
    pub(crate) available: &'data mut usize,
    pub(crate) filled: &'data mut usize,
}
#[derive(Default, Clone, Copy)]
pub struct UnfilterRegion {
    pub available: usize,
    pub filled: usize,
}
pub(super) struct ZlibStream {
    state: Box<crate::image_codecs::deflate::Decompressor>,
    started: bool,
    ignore_adler32: bool,
}
impl ZlibStream {
    pub(crate) fn new() -> Self {
        ZlibStream {
            state: Box::new(Decompressor::new()),
            started: false,
            ignore_adler32: true,
        }
    }
    pub(crate) fn reset(&mut self) {
        self.started = false;
        *self.state = Decompressor::new();
    }
    pub(crate) fn set_ignore_adler32(&mut self, flag: bool) -> bool {
        if !self.started {
            self.ignore_adler32 = flag;
            true
        } else {
            false
        }
    }
    pub(crate) fn ignore_adler32(&self) -> bool {
        self.ignore_adler32
    }
    pub(crate) fn decompress(
        &mut self,
        data: &[u8],
        image_data: &mut UnfilterBuf<'_>,
    ) -> Result<usize, DecodingError> {
        if self.state.is_done() {
            return Ok(data.len());
        }
        if !self.started && self.ignore_adler32 {
            self.state.ignore_adler32();
        }
        let in_consumed = image_data.decompress(&mut self.state, data)?;
        self.started = true;
        Ok(in_consumed)
    }
    pub(crate) fn finish(
        &mut self,
        image_data: &mut UnfilterBuf<'_>,
    ) -> Result<bool, DecodingError> {
        if !self.started || self.state.is_done() {
            return Ok(true);
        }
        if *image_data.filled == image_data.buffer.len() {
            return Ok(true);
        }
        let (_, out_consumed) = self
            .state
            .read(
                &[],
                &mut image_data.buffer[*image_data.available..],
                *image_data.filled - *image_data.available,
                false,
            )
            .map_err(|err| {
                DecodingError::Format(FormatErrorInner::CorruptFlateStream { err }.into())
            })?;
        *image_data.filled += out_consumed;
        if self.state.is_done() {
            *image_data.available = *image_data.filled;
            return Ok(true);
        }
        if *image_data.filled == image_data.buffer.len() {
            *image_data.available =
                (*image_data.available).max(image_data.filled.saturating_sub(LOOKBACK_SIZE));
            return Ok(false);
        }
        Err(DecodingError::Format(
            FormatErrorInner::CorruptFlateStream {
                err: crate::image_codecs::deflate::DecompressionError::InsufficientInput,
            }
            .into(),
        ))
    }
}
impl UnfilterRegion {}
impl UnfilterBuf<'_> {
    #[inline]
    fn decompress(
        &mut self,
        decompressor: &mut crate::image_codecs::deflate::Decompressor,
        input: &[u8],
    ) -> Result<usize, DecodingError> {
        let output_limit = (*self.filled + UnfilteringBuffer::GROWTH_BYTES).min(self.buffer.len());
        let (in_consumed, out_consumed) = decompressor
            .read(
                input,
                &mut self.buffer[*self.available..output_limit],
                *self.filled - *self.available,
                false,
            )
            .map_err(|err| {
                DecodingError::Format(FormatErrorInner::CorruptFlateStream { err }.into())
            })?;
        *self.filled += out_consumed;
        if decompressor.is_done() {
            *self.available = *self.filled;
        } else if let Some(new_available) = self.filled.checked_sub(LOOKBACK_SIZE) {
            if new_available > *self.available {
                *self.available = new_available;
            }
        }
        Ok(in_consumed)
    }
}
