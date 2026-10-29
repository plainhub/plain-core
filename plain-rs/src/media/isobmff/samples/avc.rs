use super::super::{atoms, child, u32_at};

pub(super) struct Config {
    pub width: u32,
    pub height: u32,
    pub length_size: usize,
    pub parameter_sets: Vec<Vec<u8>>,
    pub description: u32,
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
        if &entry.kind != b"avc1" {
            continue;
        }
        let width = u32::from(u16::from_be_bytes(entry.data.get(24..26)?.try_into().ok()?)).max(1);
        let height = u32::from(u16::from_be_bytes(entry.data.get(26..28)?.try_into().ok()?)).max(1);
        let avcc = child(entry.data.get(78..)?, b"avcC")?;
        if *avcc.first()? != 1 {
            return None;
        }
        let length_size = usize::from(*avcc.get(4)? & 3) + 1;
        let sps = *avcc.get(5)? & 31;
        let mut rest = avcc.get(6..)?;
        let mut parameter_sets = Vec::new();
        sets(&mut rest, sps, &mut parameter_sets)?;
        let pps = *rest.first()?;
        rest = rest.get(1..)?;
        sets(&mut rest, pps, &mut parameter_sets)?;
        return Some(Some(Config {
            width,
            height,
            length_size,
            parameter_sets,
            description: u32::try_from(index).ok()?.checked_add(1)?,
        }));
    }
    Some(None)
}

fn sets(rest: &mut &[u8], count: u8, output: &mut Vec<Vec<u8>>) -> Option<()> {
    for _ in 0..count {
        let size = u16::from_be_bytes(rest.get(..2)?.try_into().ok()?) as usize;
        let end = 2usize.checked_add(size)?;
        output.push(rest.get(2..end)?.to_vec());
        *rest = rest.get(end..)?;
    }
    Some(())
}
