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
    fn finish(mut self) -> Vec<u8> {
        self.0.push(true);
        while self.0.len() % 8 != 0 {
            self.0.push(false);
        }
        let mut output = vec![0x67];
        let mut zeros = 0;
        for group in self.0.chunks_exact(8) {
            let byte = group
                .iter()
                .fold(0, |value, bit| (value << 1) | u8::from(*bit));
            if zeros == 2 && byte <= 3 {
                output.push(3);
                zeros = 0;
            }
            output.push(byte);
            zeros = if byte == 0 { zeros + 1 } else { 0 };
        }
        output
    }
}
fn fixture(
    profile: u32,
    chroma: u32,
    frame_only: bool,
    scaling: bool,
    crop: [u32; 4],
    fixed: bool,
) -> Vec<u8> {
    let mut w = Writer::default();
    w.bits(profile, 8);
    w.bits(0, 8);
    w.bits(30, 8);
    w.unsigned(0);
    if profile != 66 {
        w.unsigned(chroma);
        if chroma == 3 {
            w.bits(1, 1);
        }
        w.unsigned(0);
        w.unsigned(0);
        w.bits(0, 1);
        w.bits(u32::from(scaling), 1);
        if scaling {
            for index in 0..if chroma == 3 { 12 } else { 8 } {
                w.bits(u32::from(index == 0), 1);
                if index == 0 {
                    for _ in 0..16 {
                        w.unsigned(0);
                    }
                }
            }
        }
    }
    w.unsigned(0);
    w.unsigned(0);
    w.unsigned(0);
    w.unsigned(1);
    w.bits(0, 1);
    w.unsigned(39);
    w.unsigned(29);
    w.bits(u32::from(frame_only), 1);
    if !frame_only {
        w.bits(0, 1);
    }
    w.bits(1, 1);
    w.bits(1, 1);
    for value in crop {
        w.unsigned(value);
    }
    w.bits(1, 1);
    w.bits(0, 1);
    w.bits(0, 1);
    w.bits(0, 1);
    w.bits(0, 1);
    w.bits(1, 1);
    w.bits(1001, 32);
    w.bits(60000, 32);
    w.bits(u32::from(fixed), 1);
    w.finish()
}
#[test]
fn baseline_high_and_separate_plane_profiles_keep_crop_units_and_fractional_rate() {
    for (profile, chroma, frame_only, scaling, width, height) in [
        (66, 1, true, false, 636, 472),
        (100, 1, true, true, 636, 472),
        (100, 1, false, false, 636, 944),
        (244, 3, true, true, 638, 476),
    ] {
        let info = sps(&fixture(
            profile,
            chroma,
            frame_only,
            scaling,
            [1, 1, 2, 2],
            true,
        ))
        .unwrap();
        assert_eq!((info.width, info.height), (width, height));
        assert!((info.frame_rate.unwrap() - 29.97002997002997).abs() < 1e-9);
    }
    assert!(
        sps(&fixture(66, 1, true, false, [0; 4], false))
            .unwrap()
            .frame_rate
            .is_none()
    );
}
#[test]
fn avc_configuration_uses_length_bounded_sps_without_decoding_frames() {
    let nal = fixture(100, 1, true, false, [0; 4], true);
    let mut config = vec![1, 100, 0, 30, 255, 225];
    config.extend((nal.len() as u16).to_be_bytes());
    config.extend(&nal);
    let info = avcc(&config).unwrap();
    assert_eq!((info.width, info.height), (640, 480));
    for end in 0..8 {
        assert!(avcc(&config[..end]).is_none());
    }
    let mut invalid = config.clone();
    invalid[6..8].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(avcc(&invalid).is_none());
    config[4] = 254;
    assert!(avcc(&config).is_none());
}
#[test]
fn malformed_and_hostile_sps_fields_do_not_overflow_or_allocate_unboundedly() {
    for bytes in [
        vec![],
        vec![0x67],
        vec![0x67; 65537],
        vec![0x67, 0, 0, 3],
        vec![0x67, 0, 0, 3, 4],
        vec![0x67, 66, 0, 30, 0, 0, 0, 0, 0],
    ] {
        assert!(sps(&bytes).is_none());
    }
    assert!(sps(&fixture(66, 1, true, false, [400, 0, 0, 0], true)).is_none());
    let mut nal = fixture(66, 1, true, false, [0; 4], true);
    nal[0] |= 128;
    assert!(sps(&nal).is_none());
}
