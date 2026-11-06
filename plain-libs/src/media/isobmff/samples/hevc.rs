use super::super::{atoms, child, u32_at};
use super::avc::Config;
use crate::media::hevc_config::NalArrays;

pub(in crate::media::isobmff) fn configuration(
    data: &[u8],
) -> Option<(usize, Vec<Vec<u8>>, Vec<(usize, u8)>)> {
    if data.len() < 23 || data[0] != 1 {
        return None;
    }
    let length_size = usize::from(data[21] & 3) + 1;
    if length_size == 3 {
        return None;
    }
    let mut sets = Vec::new();
    let mut patches = Vec::new();
    let mut counts = [0usize; 3];
    let mut offset = 23;
    for array in NalArrays::new(&data[23..], data[22]) {
        let array = array.ok()?;
        let header = array.bytes()[0];
        let kind = header & 63;
        if (32..=34).contains(&kind) {
            for nal in array.nals() {
                if nal.len() < 2
                    || nal[0] & 0x80 != 0
                    || (nal[0] >> 1) & 63 != kind
                    || nal[0] & 1 != 0
                    || nal[1] & 0xf8 != 0
                    || nal[1] & 7 == 0
                {
                    return None;
                }
                let index = usize::from(kind - 32);
                counts[index] += 1;
                // HEVC has 16 VPS IDs, 16 SPS IDs and 64 PPS IDs for the base layer.
                if counts[index] > [16, 16, 64][index] {
                    return None;
                }
                sets.push(nal.to_vec());
            }
        }
        patches.push((
            offset,
            if (32..=34).contains(&kind) {
                header | 0x80
            } else {
                header & 0x7f
            },
        ));
        offset += array.bytes().len();
    }
    if offset != data.len() || counts.contains(&0) {
        return None;
    }
    Some((length_size, sets, patches))
}

pub(super) fn read(stsd: &[u8]) -> Option<Option<Config>> {
    if u32_at(stsd, 0)? != 0 {
        return None;
    }
    let entries = atoms(stsd.get(8..)?)?;
    if u32_at(stsd, 4)? as usize != entries.len() {
        return None;
    }
    for (index, entry) in entries.iter().enumerate() {
        if !matches!(&entry.kind, b"hev1" | b"hvc1") {
            continue;
        }
        let width = u32::from(u16::from_be_bytes(entry.data.get(24..26)?.try_into().ok()?));
        let height = u32::from(u16::from_be_bytes(entry.data.get(26..28)?.try_into().ok()?));
        if width == 0 || height == 0 {
            return None;
        }
        let data = child(entry.data.get(78..)?, b"hvcC")?;
        let (length_size, parameter_sets, hevc_headers) = configuration(data)?;
        return Some(Some(Config {
            sample_entry: entry.kind,
            hevc_headers,
            width,
            height,
            length_size,
            parameter_sets,
            description: u32::try_from(index).ok()?.checked_add(1)?,
        }));
    }
    Some(None)
}
