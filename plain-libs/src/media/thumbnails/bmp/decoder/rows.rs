use super::*;
use crate::media::thumbnails::scale::{self, Bitmap, BoxRowFilter};

fn source_channels(format: &FormatFullBytes) -> usize {
    if *format == FormatFullBytes::RGB24 {
        3
    } else {
        4
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/media/thumbnails/bmp/decoder/rows.rs"]
mod tests;

fn rgb_pixel(source: &[u8], output: &mut [u8], format: &FormatFullBytes) {
    let start = usize::from(*format == FormatFullBytes::Format888);
    output[0] = source[start + 2];
    output[1] = source[start + 1];
    output[2] = source[start];
    if output.len() == 4 {
        output[3] = source[3];
    }
}

fn add_rgb<const CHANNELS: usize, const SOURCE: usize, const START: usize>(
    sums: &mut [u32],
    source: &[u8],
) {
    for (sum, source) in sums
        .chunks_exact_mut(CHANNELS)
        .zip(source.chunks_exact(SOURCE))
    {
        sum[0] += u32::from(source[START + 2]);
        sum[1] += u32::from(source[START + 1]);
        sum[2] += u32::from(source[START]);
        if CHANNELS == 4 {
            sum[3] += u32::from(source[3]);
        }
    }
}

impl<R: BufRead + Seek> BmpDecoder<R> {
    pub(super) fn read_full_byte_pixel_data(
        &mut self,
        output: &mut [u8],
        format: &FormatFullBytes,
    ) -> ImageResult<()> {
        let channels = self.num_channels();
        let source_channels = source_channels(format);
        let stride = (self.width as usize * source_channels + 3) & !3;
        let mut source = vec![0; stride];
        self.reader.seek(SeekFrom::Start(self.data_offset))?;
        let reader = &mut self.reader;
        with_rows(
            output,
            self.width,
            self.height,
            channels,
            self.top_down,
            |row| {
                reader.read_exact(&mut source)?;
                for (pixel, source) in row
                    .chunks_exact_mut(channels)
                    .zip(source.chunks_exact(source_channels))
                {
                    rgb_pixel(source, pixel, format);
                }
                Ok(())
            },
        )?;
        Ok(())
    }

    pub(in super::super) fn read_scaled(
        &mut self,
        width: u32,
        height: u32,
        crop: Option<(u32, u32)>,
    ) -> ImageResult<Option<Bitmap>> {
        let format = match self.image_type {
            ImageType::RGB24 => FormatFullBytes::RGB24,
            ImageType::RGB32 => FormatFullBytes::RGB32,
            ImageType::Bitfields32 if self.bitfields == Some(R8_G8_B8_A8_COLOR_MASK) => {
                FormatFullBytes::RGBA32
            }
            ImageType::Bitfields32 if self.bitfields == Some(R8_G8_B8_COLOR_MASK) => {
                FormatFullBytes::Format888
            }
            _ => return Ok(None),
        };
        let (cw, ch) = scale::crop_size(self.width as u32, self.height as u32, crop);
        let factor = (cw / width).min(ch / height).min(64);
        if factor < 2 {
            return Ok(None);
        }
        let channels = self.num_channels();
        let source_channels = source_channels(&format);
        let stride = (self.width as u64 * source_channels as u64 + 3) & !3;
        let end = self.data_offset + stride * self.height as u64;
        if self.reader.seek(SeekFrom::End(0))? < end {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }
        let left = (self.width as u32 - cw) / 2;
        let top = (self.height as u32 - ch) / 2;
        let physical_row = if self.top_down {
            top
        } else {
            self.height as u32 - top - ch
        };
        let ow = cw.div_ceil(factor);
        let oh = ch.div_ceil(factor);
        let mut output = vec![0; ow as usize * oh as usize * channels];
        let mut source = vec![0; cw as usize * source_channels];
        let mut vertical = vec![0u32; cw as usize * channels];
        let mut row = Vec::with_capacity(ow as usize * channels);
        let filter = BoxRowFilter::new(cw, factor);
        self.reader.seek(SeekFrom::Start(
            self.data_offset
                + u64::from(physical_row) * stride
                + u64::from(left) * source_channels as u64,
        ))?;
        let skip = stride - source.len() as u64;
        for group in 0..oh {
            let rows = if self.top_down {
                (ch - group * factor).min(factor)
            } else if group == 0 && ch % factor != 0 {
                ch % factor
            } else {
                factor
            };
            vertical.fill(0);
            for _ in 0..rows {
                self.reader.read_exact(&mut source)?;
                if skip != 0 {
                    self.reader.seek_relative(skip as i64)?;
                }
                match format {
                    FormatFullBytes::RGB24 => add_rgb::<3, 3, 0>(&mut vertical, &source),
                    FormatFullBytes::RGB32 => add_rgb::<3, 4, 0>(&mut vertical, &source),
                    FormatFullBytes::RGBA32 => add_rgb::<4, 4, 0>(&mut vertical, &source),
                    FormatFullBytes::Format888 => add_rgb::<3, 4, 1>(&mut vertical, &source),
                }
            }
            row.clear();
            if channels == 4 {
                filter.append::<4>(&mut vertical, rows, &mut row);
            } else {
                filter.append::<3>(&mut vertical, rows, &mut row);
            }
            let logical = if self.top_down { group } else { oh - 1 - group };
            let start = logical as usize * row.len();
            output[start..start + row.len()].copy_from_slice(&row);
        }
        Ok(Some(if channels == 4 {
            Bitmap::new_rgba(output, ow, oh)
        } else {
            Bitmap::new_rgb(output, ow, oh)
        }))
    }
}
