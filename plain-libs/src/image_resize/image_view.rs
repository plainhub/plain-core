use std::num::NonZeroU32;

use crate::image_resize::images::{RowsMut, TypedCroppedImage};
use crate::image_resize::pixels::InnerPixel;
use crate::image_resize::{ArrayChunks, ImageError, PixelTrait, PixelType};

pub unsafe trait ImageView: Sync + Send + Sized {
    type Pixel: InnerPixel;

    fn width(&self) -> u32;

    fn height(&self) -> u32;

    fn iter_rows(&self, start_row: u32) -> impl Iterator<Item = &[Self::Pixel]>;

    fn iter_2_rows(
        &self,
        start_y: u32,
        max_rows: u32,
    ) -> ArrayChunks<impl Iterator<Item = &[Self::Pixel]>, 2> {
        ArrayChunks::new(self.iter_rows(start_y).take(max_rows as usize))
    }

    fn iter_4_rows(
        &self,
        start_y: u32,
        max_rows: u32,
    ) -> ArrayChunks<impl Iterator<Item = &[Self::Pixel]>, 4> {
        ArrayChunks::new(self.iter_rows(start_y).take(max_rows as usize))
    }

    fn iter_rows_with_step(
        &self,
        start_y: f64,
        step: f64,
        max_rows: u32,
    ) -> impl Iterator<Item = &[Self::Pixel]> {
        let steps = (self.height() as f64 - start_y) / step;
        let steps = (steps.max(0.).ceil() as usize).min(max_rows as usize);
        let mut rows = self.iter_rows(start_y as u32);
        let mut y = start_y;
        let mut next_row_y = start_y as usize;
        let mut cur_row = None;
        (0..steps).filter_map(move |_| {
            let req_row_y = y as usize;
            if next_row_y <= req_row_y {
                for _ in next_row_y..=req_row_y {
                    cur_row = rows.next();
                }
                next_row_y = req_row_y + 1;
            }
            y += step;
            cur_row
        })
    }

    fn split_by_height(
        &self,
        start_row: u32,
        height: NonZeroU32,
        num_parts: NonZeroU32,
    ) -> Option<Vec<impl ImageView<Pixel = Self::Pixel>>> {
        let height = height.get();
        let num_parts = num_parts.get();
        if num_parts > height || height > self.height() || start_row > self.height() - height {
            return None;
        }
        let mut res = Vec::with_capacity(num_parts as usize);
        let step = height / num_parts;
        let mut modulo = height % num_parts;
        let mut top = start_row;
        let width = self.width();
        for _ in 0..num_parts {
            let mut part_height = step;
            if modulo > 0 {
                part_height += 1;
                modulo -= 1;
            }
            let image = TypedCroppedImage::from_ref(self, 0, top, width, part_height).unwrap();
            res.push(image);
            top += part_height;
        }
        debug_assert!(top - start_row == height);
        Some(res)
    }

    fn split_by_width(
        &self,
        start_col: u32,
        width: NonZeroU32,
        num_parts: NonZeroU32,
    ) -> Option<Vec<impl ImageView<Pixel = Self::Pixel>>> {
        let width = width.get();
        let num_parts = num_parts.get();
        if num_parts > width || width > self.width() || start_col > self.width() - width {
            return None;
        }
        let mut res = Vec::with_capacity(num_parts as usize);
        let step = width / num_parts;
        let mut modulo = width % num_parts;
        let mut left = start_col;
        let height = self.height();
        for _ in 0..num_parts {
            let mut part_width = step;
            if modulo > 0 {
                part_width += 1;
                modulo -= 1;
            }
            let image = TypedCroppedImage::from_ref(self, left, 0, part_width, height).unwrap();
            res.push(image);
            left += part_width;
        }
        debug_assert!(left - start_col == width);
        Some(res)
    }
}

pub unsafe trait ImageViewMut: ImageView {
    fn iter_rows_mut(&mut self, start_row: u32) -> impl Iterator<Item = &mut [Self::Pixel]>;

    fn iter_2_rows_mut(&mut self) -> ArrayChunks<impl Iterator<Item = &mut [Self::Pixel]>, 2> {
        ArrayChunks::new(self.iter_rows_mut(0))
    }

    fn iter_4_rows_mut(&mut self) -> ArrayChunks<impl Iterator<Item = &mut [Self::Pixel]>, 4> {
        ArrayChunks::new(self.iter_rows_mut(0))
    }

    fn split_by_height_mut(
        &mut self,
        start_row: u32,
        height: NonZeroU32,
        num_parts: NonZeroU32,
    ) -> Option<Vec<impl ImageViewMut<Pixel = Self::Pixel>>> {
        let height = height.get();
        let num_parts = num_parts.get();
        if num_parts > height || height > self.height() || start_row > self.height() - height {
            return None;
        }
        let width = self.width();
        let step = height / num_parts;
        let extra = height % num_parts;
        let mut rows = self.iter_rows_mut(start_row).take(height as usize);
        Some(
            (0..num_parts)
                .map(|index| {
                    let count = step + u32::from(index < extra);
                    RowsMut {
                        width,
                        rows: rows.by_ref().take(count as usize).collect(),
                    }
                })
                .collect(),
        )
    }

    fn split_by_width_mut(
        &mut self,
        start_col: u32,
        width: NonZeroU32,
        num_parts: NonZeroU32,
    ) -> Option<Vec<impl ImageViewMut<Pixel = Self::Pixel>>> {
        let width = width.get();
        let num_parts = num_parts.get();
        if num_parts > width || width > self.width() || start_col > self.width() - width {
            return None;
        }
        let height = self.height();
        let step = width / num_parts;
        let extra = width % num_parts;
        let mut parts: Vec<_> = (0..num_parts)
            .map(|index| RowsMut {
                width: step + u32::from(index < extra),
                rows: Vec::with_capacity(height as usize),
            })
            .collect();
        for row in self.iter_rows_mut(0).take(height as usize) {
            let mut remaining = &mut row[start_col as usize..(start_col + width) as usize];
            for part in &mut parts {
                let (pixels, rest) = remaining.split_at_mut(part.width as usize);
                part.rows.push(pixels);
                remaining = rest;
            }
        }
        Some(parts)
    }
}

pub trait IntoImageView {
    fn pixel_type(&self) -> Option<PixelType>;

    fn width(&self) -> u32;

    fn height(&self) -> u32;

    fn image_view<P: PixelTrait>(&self) -> Option<impl ImageView<Pixel = P>>;
}

pub trait IntoImageViewMut: IntoImageView {
    fn image_view_mut<P: PixelTrait>(&mut self) -> Option<impl ImageViewMut<Pixel = P>>;
}

pub(crate) fn try_pixel_type(image: &impl IntoImageView) -> Result<PixelType, ImageError> {
    image.pixel_type().ok_or(ImageError::UnsupportedPixelType)
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/image_resize/image_view.rs"]
mod tests;
