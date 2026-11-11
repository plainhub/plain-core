use crate::image::{ColorType, ImageError, ImageResult, error};
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub struct LimitSupport {}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub struct Limits {
    pub max_image_width: Option<u32>,
    pub max_image_height: Option<u32>,
    pub max_alloc: Option<u64>,
}
impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_image_width: None,
            max_image_height: None,
            max_alloc: Some(512 * 1024 * 1024),
        }
    }
}
impl Limits {
    #[must_use]
    pub fn no_limits() -> Limits {
        Limits {
            max_image_width: None,
            max_image_height: None,
            max_alloc: None,
        }
    }
    pub fn check_support(&self, _supported: &LimitSupport) -> ImageResult<()> {
        Ok(())
    }
    pub fn check_dimensions(&self, width: u32, height: u32) -> ImageResult<()> {
        if let Some(max_width) = self.max_image_width {
            if width > max_width {
                return Err(ImageError::Limits(error::LimitError::from_kind(
                    error::LimitErrorKind::DimensionError,
                )));
            }
        }
        if let Some(max_height) = self.max_image_height {
            if height > max_height {
                return Err(ImageError::Limits(error::LimitError::from_kind(
                    error::LimitErrorKind::DimensionError,
                )));
            }
        }
        Ok(())
    }
    pub fn reserve(&mut self, amount: u64) -> ImageResult<()> {
        if let Some(max_alloc) = self.max_alloc.as_mut() {
            if *max_alloc < amount {
                return Err(ImageError::Limits(error::LimitError::from_kind(
                    error::LimitErrorKind::InsufficientMemory,
                )));
            }
            *max_alloc -= amount;
        }
        Ok(())
    }
    pub fn reserve_usize(&mut self, amount: usize) -> ImageResult<()> {
        match u64::try_from(amount) {
            Ok(n) => self.reserve(n),
            Err(_) if self.max_alloc.is_some() => Err(ImageError::Limits(
                error::LimitError::from_kind(error::LimitErrorKind::InsufficientMemory),
            )),
            Err(_) => Ok(()),
        }
    }
    pub fn reserve_buffer(
        &mut self,
        width: u32,
        height: u32,
        color_type: ColorType,
    ) -> ImageResult<()> {
        self.check_dimensions(width, height)?;
        let in_memory_size = u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(color_type.bytes_per_pixel().into());
        self.reserve(in_memory_size)?;
        Ok(())
    }
    pub fn free(&mut self, amount: u64) {
        if let Some(max_alloc) = self.max_alloc.as_mut() {
            *max_alloc = max_alloc.saturating_add(amount);
        }
    }
    pub fn free_usize(&mut self, amount: usize) {
        match u64::try_from(amount) {
            Ok(n) => self.free(n),
            Err(_) if self.max_alloc.is_some() => {
                panic!("max_alloc is set, we should have exited earlier when the reserve failed");
            }
            Err(_) => {}
        }
    }
}
