use crate::image_resize::images::{TypedImage, TypedImageRef};
use crate::image_resize::pixels::InnerPixel;
use crate::image_resize::{
    ImageBufferError, ImageView, ImageViewMut, IntoImageView, IntoImageViewMut, PixelTrait,
    PixelType,
};

#[derive(Debug)]
pub struct Image {
    width: u32,
    height: u32,
    buffer: Vec<u8>,
    pixel_type: PixelType,
}

impl Image {
    pub fn new(width: u32, height: u32, pixel_type: PixelType) -> Self {
        let size = crate::image_resize::utils::image_size(width, height, pixel_type.size())
            .expect("Image dimensions exceed address space");
        let buffer = vec![0; size];
        Self {
            width,
            height,
            buffer,
            pixel_type,
        }
    }

    pub fn from_vec_u8(
        width: u32,
        height: u32,
        buffer: Vec<u8>,
        pixel_type: PixelType,
    ) -> Result<Self, ImageBufferError> {
        let size = crate::image_resize::utils::image_size(width, height, pixel_type.size())
            .ok_or(ImageBufferError::InvalidBufferSize)?;
        if buffer.len() < size {
            return Err(ImageBufferError::InvalidBufferSize);
        }
        if !pixel_type.is_aligned(&buffer) {
            return Err(ImageBufferError::InvalidBufferAlignment);
        }
        Ok(Self {
            width,
            height,
            buffer,
            pixel_type,
        })
    }
}

impl Image {
    #[inline]
    pub fn pixel_type(&self) -> PixelType {
        self.pixel_type
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    #[inline]
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        self.buffer
    }

    pub fn typed_image<P: InnerPixel>(&self) -> Option<TypedImageRef<'_, P>> {
        if P::pixel_type() != self.pixel_type {
            return None;
        }
        let typed_image =
            TypedImageRef::from_buffer(self.width, self.height, self.buffer()).unwrap();
        Some(typed_image)
    }

    pub fn typed_image_mut<P: InnerPixel>(&mut self) -> Option<TypedImage<'_, P>> {
        if P::pixel_type() != self.pixel_type {
            return None;
        }
        let typed_image =
            TypedImage::from_buffer(self.width, self.height, self.buffer_mut()).unwrap();
        Some(typed_image)
    }
}

impl IntoImageView for Image {
    fn pixel_type(&self) -> Option<PixelType> {
        Some(self.pixel_type)
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn image_view<P: PixelTrait>(&self) -> Option<impl ImageView<Pixel = P>> {
        self.typed_image()
    }
}

impl IntoImageViewMut for Image {
    fn image_view_mut<P: PixelTrait>(&mut self) -> Option<impl ImageViewMut<Pixel = P>> {
        self.typed_image_mut()
    }
}
