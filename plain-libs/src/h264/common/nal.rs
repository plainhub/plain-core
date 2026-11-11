use std::vec;
use std::vec::Vec;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NalUnitType {
    NonIdrSlice,
    IdrSlice,
    Sei,
    Sps,
    Pps,
    AccessUnitDelimiter,
    Other(u8),
}
impl NalUnitType {
    pub fn id(self) -> u8 {
        match self {
            NalUnitType::NonIdrSlice => 1,
            NalUnitType::IdrSlice => 5,
            NalUnitType::Sei => 6,
            NalUnitType::Sps => 7,
            NalUnitType::Pps => 8,
            NalUnitType::AccessUnitDelimiter => 9,
            NalUnitType::Other(v) => v & 0x1f,
        }
    }
    pub fn from_id(v: u8) -> Self {
        match v & 0x1f {
            1 => NalUnitType::NonIdrSlice,
            5 => NalUnitType::IdrSlice,
            6 => NalUnitType::Sei,
            7 => NalUnitType::Sps,
            8 => NalUnitType::Pps,
            9 => NalUnitType::AccessUnitDelimiter,
            other => NalUnitType::Other(other),
        }
    }
}

pub fn emulation_prevent_with(rbsp: &[u8], mut put: impl FnMut(u8)) {
    let mut zeros = 0usize;
    for &b in rbsp {
        if zeros >= 2 && b <= 0x03 {
            put(0x03);
            zeros = 0;
        }
        put(b);
        if b == 0 {
            zeros += 1;
        } else {
            zeros = 0;
        }
    }
}

pub fn emulation_unprevent(ebsp: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    let mut zeros = 0usize;
    let mut i = 0;
    let mut any = false;
    while i < ebsp.len() {
        let b = ebsp[i];
        if zeros >= 2 && b == 0x03 && i + 1 < ebsp.len() && ebsp[i + 1] <= 0x03 {
            any = true;
            break;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        i += 1;
    }
    if !any {
        return std::borrow::Cow::Borrowed(ebsp);
    }
    let mut out = Vec::with_capacity(ebsp.len());
    let mut zeros = 0usize;
    let mut i = 0;
    while i < ebsp.len() {
        let b = ebsp[i];
        if zeros >= 2 && b == 0x03 && i + 1 < ebsp.len() && ebsp[i + 1] <= 0x03 {
            zeros = 0;
            i += 1;
            continue;
        }
        out.push(b);
        if b == 0 {
            zeros += 1;
        } else {
            zeros = 0;
        }
        i += 1;
    }
    std::borrow::Cow::Owned(out)
}
pub fn split_annex_b(stream: &[u8]) -> Vec<&[u8]> {
    let mut nals = Vec::new();
    let mut prev: Option<usize> = None;
    let mut i = 0;
    while let Some(w) = stream.get(i..i + 3) {
        if w[0] == 0 && w[1] == 0 && w[2] == 1 {
            if let Some(s) = prev {
                let mut end = i;
                if end > s && stream.get(end - 1) == Some(&0) {
                    end -= 1;
                }
                if end > s {
                    nals.push(&stream[s..end]);
                }
            }
            prev = Some(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    if let Some(s) = prev {
        if stream.len() > s {
            nals.push(&stream[s..]);
        }
    }
    nals
}
