use crate::image_resize::{ImageView, ImageViewMut, pixels::InnerPixel};

pub(crate) struct RowsMut<'a, P> {
    pub(crate) width: u32,
    pub(crate) rows: Vec<&'a mut [P]>,
}

// The rows come from disjoint slice partitions and each row has exactly width pixels.
unsafe impl<P: InnerPixel> ImageView for RowsMut<'_, P> {
    type Pixel = P;
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.rows.len() as u32
    }
    fn iter_rows(&self, start_row: u32) -> impl Iterator<Item = &[P]> {
        self.rows.iter().skip(start_row as usize).map(|row| &**row)
    }
}

// Reborrowing stored row slices preserves the exclusivity established by split_at_mut.
unsafe impl<P: InnerPixel> ImageViewMut for RowsMut<'_, P> {
    fn iter_rows_mut(&mut self, start_row: u32) -> impl Iterator<Item = &mut [P]> {
        self.rows
            .iter_mut()
            .skip(start_row as usize)
            .map(|row| &mut **row)
    }
}
