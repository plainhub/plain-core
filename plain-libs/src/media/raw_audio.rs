use std::io::{Read, Seek, SeekFrom};

pub(super) fn milliseconds(reader: &mut (impl Read + Seek)) -> Option<i64> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    let mut prefix = [0; 16];
    let available = usize::try_from(length.min(16)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    reader.read_exact(&mut prefix[..available]).ok()?;
    let bytes = &prefix[..available];
    if bytes.starts_with(b"MThd") {
        return super::midi::milliseconds(reader);
    }
    if bytes == super::asf::HEADER.as_slice() {
        return super::asf::milliseconds(reader);
    }
    let amr = [
        (&b"#!AMR-WB_MC1.0\n"[..], true, true),
        (&b"#!AMR_MC1.0\n"[..], false, true),
        (&b"#!AMR-WB\n"[..], true, false),
        (&b"#!AMR\n"[..], false, false),
    ]
    .into_iter()
    .find(|(magic, _, _)| bytes.starts_with(magic));
    if let Some((magic, wide, multi)) = amr {
        let mut offset = magic.len() as u64;
        let channels = if multi {
            let mut channel = [0; 4];
            reader.seek(SeekFrom::Start(offset)).ok()?;
            reader.read_exact(&mut channel).ok()?;
            offset += 4;
            u64::from(u32::from_be_bytes(channel) & 15)
        } else {
            1
        };
        if channels == 0 {
            return None;
        }
        return amr_milliseconds(reader, offset, length, wide, channels);
    }
    let offset = if bytes.starts_with(b"ID3") {
        if available < 10
            || !matches!(prefix[3], 2..=4)
            || prefix[6..10].iter().any(|byte| byte & 128 != 0)
        {
            return None;
        }
        let size = prefix[6..10]
            .iter()
            .fold(0u64, |value, byte| (value << 7) | u64::from(*byte));
        10u64
            .checked_add(size)?
            .checked_add(if prefix[3] == 4 && prefix[5] & 16 != 0 {
                10
            } else {
                0
            })?
    } else {
        0
    };
    adts_milliseconds(reader, offset, length)
}

fn amr_milliseconds(
    reader: &mut (impl Read + Seek),
    mut offset: u64,
    end: u64,
    wide: bool,
    channels: u64,
) -> Option<i64> {
    const NARROW: [u64; 16] = [13, 14, 16, 18, 20, 21, 27, 32, 6, 0, 0, 0, 0, 0, 0, 1];
    const WIDE: [u64; 16] = [18, 24, 33, 37, 41, 47, 51, 59, 61, 6, 0, 0, 0, 0, 1, 1];
    let sizes = if wide { WIDE } else { NARROW };
    let mut frames = 0u64;
    while offset < end {
        reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0];
        reader.read_exact(&mut header).ok()?;
        let size = sizes[usize::from((header[0] >> 3) & 15)];
        if size == 0 {
            return None;
        }
        offset = offset.checked_add(size)?;
        if offset > end {
            return None;
        }
        frames = frames.checked_add(1)?;
    }
    if frames % channels != 0 {
        return None;
    }
    i64::try_from((frames / channels).checked_mul(20)?).ok()
}

fn adts_milliseconds(reader: &mut (impl Read + Seek), mut offset: u64, end: u64) -> Option<i64> {
    const SAMPLE_RATES: [u128; 13] = [
        96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
    ];
    const TICKS_PER_SECOND: u128 = 28_224_000;
    let mut ticks = 0u128;
    let mut frames = 0u64;
    while offset < end {
        if end.checked_sub(offset)? < 7 {
            return None;
        }
        reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut bytes = [0; 7];
        reader.read_exact(&mut bytes).ok()?;
        if &bytes[..3] == b"TAG" && end - offset == 128 {
            break;
        }
        if bytes[0] != 0xff || bytes[1] & 0xf6 != 0xf0 {
            return None;
        }
        let rate = *SAMPLE_RATES.get(usize::from((bytes[2] >> 2) & 15))?;
        let size =
            (u64::from(bytes[3] & 3) << 11) | (u64::from(bytes[4]) << 3) | u64::from(bytes[5] >> 5);
        if size < if bytes[1] & 1 == 0 { 9 } else { 7 } {
            return None;
        }
        offset = offset.checked_add(size)?;
        if offset > end {
            return None;
        }
        let samples = 1024 * u128::from((bytes[6] & 3) + 1);
        ticks = ticks.checked_add(samples.checked_mul(TICKS_PER_SECOND / rate)?)?;
        frames = frames.checked_add(1)?;
    }
    if frames == 0 {
        return None;
    }
    i64::try_from(ticks.checked_mul(1000)? / TICKS_PER_SECOND).ok()
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/raw_audio.rs"]
mod tests;
