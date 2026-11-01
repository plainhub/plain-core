use super::bits::Bits;

use super::video_info::StreamInfo;

fn scaling_list(bits: &mut Bits<'_>, size: usize) -> Option<()> {
    let mut last = 8i64;
    let mut next = 8i64;
    for _ in 0..size {
        if next != 0 {
            next = (last + bits.signed()?).rem_euclid(256);
        }
        if next != 0 {
            last = next;
        }
    }
    Some(())
}
fn vui(bits: &mut Bits<'_>) -> Option<Option<f64>> {
    if !bits.flag()? {
        return Some(None);
    }
    if bits.flag()? && bits.read(8)? == 255 {
        bits.read(16)?;
        bits.read(16)?;
    }
    if bits.flag()? {
        bits.read(1)?;
    }
    if bits.flag()? {
        bits.read(3)?;
        bits.read(1)?;
        if bits.flag()? {
            bits.read(24)?;
        }
    }
    if bits.flag()? {
        bits.unsigned()?;
        bits.unsigned()?;
    }
    if !bits.flag()? {
        return Some(None);
    }
    let units = bits.read(32)?;
    let scale = bits.read(32)?;
    let fixed = bits.flag()?;
    if units == 0 || scale == 0 {
        return None;
    }
    Some(fixed.then(|| f64::from(scale) / (2.0 * f64::from(units))))
}
pub(super) fn sps(nal: &[u8]) -> Option<StreamInfo> {
    if nal.len() > 64 * 1024 || nal.first()? & 0x9f != 7 {
        return None;
    }
    let bytes = super::bits::rbsp(&nal[1..])?;
    let mut bits = Bits::new(&bytes);
    let profile = bits.read(8)?;
    bits.read(16)?;
    if bits.unsigned()? > 31 {
        return None;
    }
    let mut chroma = 1;
    if matches!(
        profile,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        chroma = bits.unsigned()?;
        if chroma > 3 {
            return None;
        }
        let separate = chroma == 3 && bits.flag()?;
        if bits.unsigned()? > 6 || bits.unsigned()? > 6 {
            return None;
        }
        bits.read(1)?;
        if bits.flag()? {
            for index in 0..if chroma == 3 { 12 } else { 8 } {
                if bits.flag()? {
                    scaling_list(&mut bits, if index < 6 { 16 } else { 64 })?;
                }
            }
        }
        if separate {
            chroma = 0;
        }
    }
    if bits.unsigned()? > 12 {
        return None;
    }
    match bits.unsigned()? {
        0 => {
            if bits.unsigned()? > 12 {
                return None;
            }
        }
        1 => {
            bits.read(1)?;
            bits.signed()?;
            bits.signed()?;
            let count = bits.unsigned()?;
            if count > 255 {
                return None;
            }
            for _ in 0..count {
                bits.signed()?;
            }
        }
        2 => {}
        _ => return None,
    }
    bits.unsigned()?;
    bits.read(1)?;
    let columns = bits.unsigned()?.checked_add(1)?;
    let rows = bits.unsigned()?.checked_add(1)?;
    if columns > 4096 || rows > 4096 {
        return None;
    }
    let frame_only = bits.flag()?;
    if !frame_only {
        bits.read(1)?;
    }
    bits.read(1)?;
    let (left, right, top, bottom) = if bits.flag()? {
        (
            bits.unsigned()?,
            bits.unsigned()?,
            bits.unsigned()?,
            bits.unsigned()?,
        )
    } else {
        (0, 0, 0, 0)
    };
    let frame_factor = if frame_only { 1 } else { 2 };
    let crop_x = if matches!(chroma, 1 | 2) { 2 } else { 1 };
    let crop_y = frame_factor * if chroma == 1 { 2 } else { 1 };
    let width = columns
        .checked_mul(16)?
        .checked_sub(left.checked_add(right)?.checked_mul(crop_x)?)?;
    let height = rows
        .checked_mul(16)?
        .checked_mul(frame_factor)?
        .checked_sub(top.checked_add(bottom)?.checked_mul(crop_y)?)?;
    if width == 0 || height == 0 {
        return None;
    }
    Some(StreamInfo {
        width,
        height,
        frame_rate: vui(&mut bits)?,
    })
}
pub(super) fn avcc(data: &[u8]) -> Option<StreamInfo> {
    if *data.first()? != 1 || data.len() < 6 || data[4] & 3 == 2 {
        return None;
    }
    let mut offset = 6usize;
    for _ in 0..(data[5] & 31) {
        let size = usize::from(u16::from_be_bytes(
            data.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ));
        offset += 2;
        let end = offset.checked_add(size)?;
        if let Some(info) = sps(data.get(offset..end)?) {
            return Some(info);
        }
        offset = end;
    }
    None
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/h264.rs"]
mod tests;
