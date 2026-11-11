use crate::image::{GenericImageView, error};
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(serde::Serialize, serde::Deserialize))]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Rect {
    pub(crate) fn from_image_at(image: &(impl GenericImageView + ?Sized), x: u32, y: u32) -> Self {
        Self {
            x,
            y,
            width: image.width(),
            height: image.height(),
        }
    }
    pub(crate) fn test_in_bounds(
        &self,
        image: &(impl GenericImageView + ?Sized),
    ) -> Result<(), error::ImageError> {
        if image.width().checked_sub(self.width) >= Some(self.x)
            && image.height().checked_sub(self.height) >= Some(self.y)
        {
            Ok(())
        } else {
            Err(error::ImageError::Parameter(
                error::ParameterError::from_kind(error::ParameterErrorKind::DimensionMismatch),
            ))
        }
    }
}
