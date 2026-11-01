use crate::media::bits::Bits;

pub(super) fn dimensions(codec: u8, data: &[u8]) -> Option<(u32, u32)> {
    let mut bits = Bits::new(data);
    let (width, height) = match codec {
        1 => {
            let (width, height) = crate::utils::image_dimensions::dimensions(data)?;
            (u32::try_from(width).ok()?, u32::try_from(height).ok()?)
        }
        2 => {
            if bits.read(17)? != 1 || bits.read(5)? > 1 {
                return None;
            }
            bits.read(8)?;
            match bits.read(3)? {
                0 => (bits.read(8)?, bits.read(8)?),
                1 => (bits.read(16)?, bits.read(16)?),
                2 => (352, 288),
                3 => (176, 144),
                4 => (128, 96),
                5 => (320, 240),
                6 => (160, 120),
                _ => return None,
            }
        }
        3 | 6 => {
            bits.read(4)?;
            let width = bits.read(12)?;
            bits.read(4)?;
            (width, bits.read(12)?)
        }
        _ => return None,
    };
    (width > 0 && height > 0).then_some((width, height))
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/flv/video.rs"]
mod tests;
