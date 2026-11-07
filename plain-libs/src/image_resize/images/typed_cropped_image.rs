use crate::image_resize::{CropBoxError, ImageView};

pub(crate) fn check_crop_box(
    img_width: u32,
    img_height: u32,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<(), CropBoxError> {
    if left >= img_width || top >= img_height {
        return Err(CropBoxError::PositionIsOutOfImageBoundaries);
    }
    if width > img_width - left || height > img_height - top {
        return Err(CropBoxError::SizeIsOutOfImageBoundaries);
    }
    Ok(())
}

pub(crate) struct TypedCroppedImage<'a, V> {
    image: &'a V,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}
impl<'a, V: ImageView> TypedCroppedImage<'a, V> {
    pub(crate) fn from_ref(
        image: &'a V,
        left: u32,
        top: u32,
        width: u32,
        height: u32,
    ) -> Result<Self, CropBoxError> {
        check_crop_box(image.width(), image.height(), left, top, width, height)?;
        Ok(Self {
            image,
            left,
            top,
            width,
            height,
        })
    }
}

// Construction validates the rectangle and row slicing retains those bounds without mutable aliases.
unsafe impl<V: ImageView> ImageView for TypedCroppedImage<'_, V> {
    type Pixel = V::Pixel;
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn iter_rows(&self, start_row: u32) -> impl Iterator<Item = &[Self::Pixel]> {
        let start = start_row.min(self.height);
        let left = self.left as usize;
        let right = left + self.width as usize;
        self.image
            .iter_rows(self.top + start)
            .take((self.height - start) as usize)
            .map(move |row| &row[left..right])
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/image_resize/images/typed_cropped_image.rs"]
mod tests;
