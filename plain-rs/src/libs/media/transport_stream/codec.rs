use super::super::{bits::Bits, h264, hevc};
use super::StreamInfo;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Codec {
    H264,
    H265,
    Mpeg,
}

pub(super) fn elementary(codec: Codec, bytes: &[u8]) -> Option<StreamInfo> {
    let mut offset = 0;
    while offset + 4 <= bytes.len() {
        if bytes.get(offset..offset + 3)? != [0, 0, 1] {
            offset += 1;
            continue;
        }
        let start = offset + 3;
        offset = start;
        if (codec == Codec::H264 && bytes[start] & 31 == 7)
            || (codec == Codec::H265 && (bytes[start] >> 1) & 63 == 33)
        {
            let end = bytes[start + 1..]
                .windows(3)
                .position(|value| value == [0, 0, 1])
                .map(|position| start + 1 + position)
                .unwrap_or(bytes.len());
            let info = if codec == Codec::H264 {
                h264::sps(&bytes[start..end])
            } else {
                hevc::sps(&bytes[start..end])
            };
            if info.is_some() {
                return info;
            }
        }
        if codec == Codec::Mpeg && bytes[start] == 0xb3 {
            let mut bits = Bits::new(bytes.get(start + 1..)?);
            let width = bits.read(12)?;
            let height = bits.read(12)?;
            bits.read(4)?;
            let rate = match bits.read(4)? {
                1 => Some(24000.0 / 1001.0),
                2 => Some(24.0),
                3 => Some(25.0),
                4 => Some(30000.0 / 1001.0),
                5 => Some(30.0),
                6 => Some(50.0),
                7 => Some(60000.0 / 1001.0),
                8 => Some(60.0),
                _ => None,
            };
            if width > 0 && height > 0 {
                return Some(StreamInfo {
                    width,
                    height,
                    frame_rate: rate,
                });
            }
        }
    }
    None
}
