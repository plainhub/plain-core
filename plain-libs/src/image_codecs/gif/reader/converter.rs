use super::decoder::{DecodingError, OutputBuffer, PLTE_CHANNELS};
use crate::image_codecs::gif::MemoryLimit;
use crate::image_codecs::gif::common::Frame;
use core::iter;
use core::mem;
use std::borrow::Cow;
use std::vec::Vec;
pub(crate) const N_CHANNELS: usize = 4;
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum ColorOutput {
    RGBA = 0,
    Indexed = 1,
}
pub(crate) type FillBufferCallback<'a> =
    &'a mut dyn FnMut(&mut OutputBuffer<'_>) -> Result<usize, DecodingError>;
pub(crate) struct PixelConverter {
    color_output: ColorOutput,
    buffer: Vec<u8>,
    global_palette: Option<Vec<u8>>,
}
impl PixelConverter {
    pub(crate) const fn new(color_output: ColorOutput) -> Self {
        Self {
            color_output,
            buffer: Vec::new(),
            global_palette: None,
        }
    }
    pub(crate) fn check_buffer_size(
        &self,
        frame: &Frame<'_>,
        memory_limit: &MemoryLimit,
    ) -> Result<usize, DecodingError> {
        let pixel_bytes = memory_limit
            .buffer_size(self.color_output, frame.width, frame.height)
            .ok_or_else(|| DecodingError::OutOfMemory)?;
        debug_assert_eq!(
            pixel_bytes,
            self.buffer_size(frame).unwrap(),
            "Checked computation diverges from required buffer size"
        );
        Ok(pixel_bytes)
    }
    #[inline]
    pub(crate) fn read_frame(
        &mut self,
        frame: &mut Frame<'_>,
        data_callback: FillBufferCallback<'_>,
        memory_limit: &MemoryLimit,
    ) -> Result<(), DecodingError> {
        let pixel_bytes = self.check_buffer_size(frame, memory_limit)?;
        let mut vec = match mem::replace(&mut frame.buffer, Cow::Borrowed(&[])) {
            Cow::Owned(mut vec) if vec.capacity() >= pixel_bytes => {
                vec.resize(pixel_bytes, 0);
                vec
            }
            _ => vec![0; pixel_bytes],
        };
        self.read_into_buffer(frame, &mut vec, data_callback)?;
        frame.buffer = Cow::Owned(vec);
        frame.interlaced = false;
        Ok(())
    }
    #[inline]
    pub(crate) const fn buffer_size(&self, frame: &Frame<'_>) -> Option<usize> {
        self.line_length(frame).checked_mul(frame.height as usize)
    }
    #[inline]
    pub(crate) const fn line_length(&self, frame: &Frame<'_>) -> usize {
        use self::ColorOutput::{Indexed, RGBA};
        match self.color_output {
            RGBA => frame.width as usize * N_CHANNELS,
            Indexed => frame.width as usize,
        }
    }
    #[inline(never)]
    pub(crate) fn fill_buffer(
        &mut self,
        current_frame: &Frame<'_>,
        mut buf: &mut [u8],
        data_callback: FillBufferCallback<'_>,
    ) -> Result<bool, DecodingError> {
        loop {
            let decode_into = match self.color_output {
                ColorOutput::Indexed => &mut buf[..],
                ColorOutput::RGBA => {
                    let buffer_size = buf.len() / N_CHANNELS;
                    if buffer_size == 0 {
                        return Err(DecodingError::format("odd-sized buffer"));
                    }
                    if self.buffer.len() < buffer_size {
                        self.buffer.resize(buffer_size, 0);
                    }
                    &mut self.buffer[..buffer_size]
                }
            };
            match data_callback(&mut OutputBuffer::Slice(decode_into))? {
                0 => return Ok(false),
                bytes_decoded => {
                    match self.color_output {
                        ColorOutput::RGBA => {
                            let transparent = current_frame.transparent;
                            let palette: &[u8] = current_frame
                                .palette
                                .as_deref()
                                .or(self.global_palette.as_deref())
                                .unwrap_or_default();
                            let (pixels, rest) = buf.split_at_mut(bytes_decoded * N_CHANNELS);
                            buf = rest;
                            for (rgba, idx) in pixels
                                .chunks_exact_mut(N_CHANNELS)
                                .zip(self.buffer.iter().copied().take(bytes_decoded))
                            {
                                let plte_offset = PLTE_CHANNELS * idx as usize;
                                if let Some(colors) =
                                    palette.get(plte_offset..plte_offset + PLTE_CHANNELS)
                                {
                                    rgba[0] = colors[0];
                                    rgba[1] = colors[1];
                                    rgba[2] = colors[2];
                                    rgba[3] = if let Some(t) = transparent {
                                        if t == idx { 0x00 } else { 0xFF }
                                    } else {
                                        0xFF
                                    };
                                }
                            }
                        }
                        ColorOutput::Indexed => {
                            buf = &mut buf[bytes_decoded..];
                        }
                    }
                    if buf.is_empty() {
                        return Ok(true);
                    }
                }
            }
        }
    }
    pub(crate) fn global_palette(&self) -> Option<&[u8]> {
        self.global_palette.as_deref()
    }
    pub(crate) fn set_global_palette(&mut self, palette: Vec<u8>) {
        self.global_palette = if !palette.is_empty() {
            Some(palette)
        } else {
            None
        };
    }
    pub(crate) fn read_into_buffer(
        &mut self,
        frame: &Frame<'_>,
        buf: &mut [u8],
        data_callback: FillBufferCallback<'_>,
    ) -> Result<(), DecodingError> {
        if frame.interlaced {
            let width = self.line_length(frame);
            for row in (InterlaceIterator {
                len: frame.height,
                next: 0,
                pass: 0,
            }) {
                let start = row * width;
                let line = buf
                    .get_mut(start..)
                    .and_then(|b| b.get_mut(..width))
                    .ok_or_else(|| DecodingError::format("buffer too small"))?;
                if !self.fill_buffer(frame, line, data_callback)? {
                    return Err(DecodingError::format("image truncated"));
                }
            }
        } else {
            let buf = self
                .buffer_size(frame)
                .and_then(|buffer_size| buf.get_mut(..buffer_size))
                .ok_or_else(|| DecodingError::format("buffer too small"))?;
            if !self.fill_buffer(frame, buf, data_callback)? {
                return Err(DecodingError::format("image truncated"));
            }
        };
        Ok(())
    }
}
struct InterlaceIterator {
    len: u16,
    next: usize,
    pass: usize,
}
impl iter::Iterator for InterlaceIterator {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        let mut next = self.next + *[8, 8, 4, 2].get(self.pass)?;
        while next >= self.len as usize {
            debug_assert!(self.pass < 4);
            next = *[4, 2, 1, 0].get(self.pass)?;
            self.pass += 1;
        }
        mem::swap(&mut next, &mut self.next);
        Some(next)
    }
}
