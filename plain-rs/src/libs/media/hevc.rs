use super::{
    bits::{self, Bits},
    video_info::StreamInfo,
};

fn profile(bits: &mut Bits<'_>, sub_layers: usize) -> Option<()> {
    bits.read(8)?;
    bits.read(32)?;
    bits.read(32)?;
    bits.read(16)?;
    bits.read(8)?;
    let mut flags = [(false, false); 7];
    for flags in flags.iter_mut().take(sub_layers) {
        *flags = (bits.flag()?, bits.flag()?);
    }
    if sub_layers > 0 {
        for _ in sub_layers..8 {
            if bits.read(2)? != 0 {
                return None;
            }
        }
    }
    for (profile, level) in flags.into_iter().take(sub_layers) {
        if profile {
            bits.read(8)?;
            bits.read(32)?;
            bits.read(32)?;
            bits.read(16)?;
        }
        if level {
            bits.read(8)?;
        }
    }
    Some(())
}
pub(super) fn sps(nal: &[u8]) -> Option<StreamInfo> {
    if nal.len() < 2
        || nal.len() > 64 * 1024
        || nal[0] & 0x80 != 0
        || (nal[0] >> 1) & 63 != 33
        || nal[1] & 7 == 0
        || nal[0] & 1 != 0
        || nal[1] & 0xf8 != 0
    {
        return None;
    }
    let data = bits::rbsp(&nal[2..])?;
    let mut bits = Bits::new(&data);
    bits.read(4)?;
    let sub_layers = bits.read(3)? as usize;
    if sub_layers > 6 {
        return None;
    }
    bits.read(1)?;
    profile(&mut bits, sub_layers)?;
    if bits.unsigned()? > 15 {
        return None;
    }
    let mut chroma = bits.unsigned()?;
    if chroma > 3 {
        return None;
    }
    if chroma == 3 && bits.flag()? {
        chroma = 0;
    }
    let width = bits.unsigned()?;
    let height = bits.unsigned()?;
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
    let crop_x = if matches!(chroma, 1 | 2) { 2 } else { 1 };
    let crop_y = if chroma == 1 { 2 } else { 1 };
    let width = width.checked_sub(left.checked_add(right)?.checked_mul(crop_x)?)?;
    let height = height.checked_sub(top.checked_add(bottom)?.checked_mul(crop_y)?)?;
    (width > 0 && height > 0).then_some(StreamInfo {
        width,
        height,
        frame_rate: None,
    })
}
pub(super) fn hvcc(data: &[u8]) -> Option<StreamInfo> {
    if data.len() < 23 || data[0] != 1 {
        return None;
    }
    let mut offset = 23usize;
    for _ in 0..data[22] {
        let kind = *data.get(offset)? & 63;
        let count = u16::from_be_bytes(
            data.get(offset + 1..offset.checked_add(3)?)?
                .try_into()
                .ok()?,
        );
        offset += 3;
        for _ in 0..count {
            let size = usize::from(u16::from_be_bytes(
                data.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
            ));
            offset += 2;
            let end = offset.checked_add(size)?;
            let nal = data.get(offset..end)?;
            if kind == 33
                && let Some(info) = sps(nal)
            {
                return Some(info);
            }
            offset = end;
        }
    }
    None
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/media/hevc.rs"]
mod tests;
