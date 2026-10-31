use super::*;

#[derive(Default)]
struct Writer(Vec<bool>);
impl Writer {
    fn bits(&mut self, value: u32, count: usize) {
        for shift in (0..count).rev() {
            self.0.push(value & (1 << shift) != 0);
        }
    }
    fn unsigned(&mut self, value: u32) {
        let value = value + 1;
        let width = 32 - value.leading_zeros();
        for _ in 1..width {
            self.0.push(false);
        }
        self.bits(value, width as usize);
    }
    fn profile(&mut self, level: bool) {
        self.bits(1, 8);
        self.bits(0, 32);
        self.bits(0, 32);
        self.bits(0, 16);
        if level {
            self.bits(120, 8);
        }
    }
    fn finish(mut self) -> Vec<u8> {
        self.0.push(true);
        while self.0.len() % 8 != 0 {
            self.0.push(false);
        }
        let mut bytes = vec![0x42, 1];
        let mut zeros = 0;
        for group in self.0.chunks_exact(8) {
            let byte = group
                .iter()
                .fold(0, |value, bit| (value << 1) | u8::from(*bit));
            if zeros == 2 && byte <= 3 {
                bytes.push(3);
                zeros = 0;
            }
            bytes.push(byte);
            zeros = if byte == 0 { zeros + 1 } else { 0 };
        }
        bytes
    }
}
fn fixture(chroma: u32, sub_layers: bool, width: u32, crop: [u32; 4]) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.bits(0, 4);
    writer.bits(if sub_layers { 2 } else { 0 }, 3);
    writer.bits(1, 1);
    writer.profile(true);
    if sub_layers {
        writer.bits(1, 1);
        writer.bits(1, 1);
        writer.bits(0, 1);
        writer.bits(1, 1);
        for _ in 2..8 {
            writer.bits(0, 2);
        }
        writer.profile(false);
        writer.bits(90, 8);
        writer.bits(60, 8);
    }
    writer.unsigned(0);
    writer.unsigned(chroma);
    if chroma == 3 {
        writer.bits(1, 1);
    }
    writer.unsigned(width);
    writer.unsigned(96);
    writer.bits(1, 1);
    for value in crop {
        writer.unsigned(value);
    }
    writer.finish()
}
#[test]
fn conformance_windows_preserve_chroma_units_and_sub_layer_profile_layouts() {
    for layers in [false, true] {
        for (chroma, width, height) in [(0, 126, 92), (1, 124, 88), (2, 124, 92), (3, 126, 92)] {
            let info = sps(&fixture(chroma, layers, 128, [1, 1, 2, 2])).unwrap();
            assert_eq!((info.width, info.height), (width, height));
            assert!(info.frame_rate.is_none());
        }
    }
}
#[test]
fn hvcc_skips_other_arrays_and_bounds_selected_parameter_sets() {
    let nal = fixture(1, false, 128, [0; 4]);
    let mut bytes = vec![0; 23];
    bytes[0] = 1;
    bytes[22] = 2;
    bytes.extend([32, 0, 1, 0, 3, 0x40, 1, 0]);
    bytes.extend([33, 0, 1]);
    bytes.extend((nal.len() as u16).to_be_bytes());
    bytes.extend(&nal);
    assert_eq!(hvcc(&bytes).unwrap().width, 128);
    bytes[34..36].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(hvcc(&bytes).is_none());
}
#[test]
fn unsupported_layers_invalid_headers_truncation_and_overflow_remain_unknown() {
    let valid = fixture(1, false, 128, [0; 4]);
    for end in 0..14 {
        assert!(sps(&valid[..end.min(valid.len())]).is_none());
    }
    for (index, value) in [(0, 0xc2), (0, 0x40), (1, 0), (1, 9), (2, 15)] {
        let mut bad = valid.clone();
        bad[index] = value;
        assert!(sps(&bad).is_none());
    }
    assert!(sps(&vec![0x42; 65537]).is_none());
    assert!(sps(&fixture(1, false, 0, [0; 4])).is_none());
    assert!(sps(&fixture(1, false, 128, [200, 0, 0, 0])).is_none());
}
