use super::*;

fn sorenson(format: u32, width: u32, height: u32) -> Vec<u8> {
    let mut bits = Vec::new();
    let mut write = |value: u32, count: usize| {
        for shift in (0..count).rev() {
            bits.push(value & (1 << shift) != 0);
        }
    };
    write(1, 17);
    write(1, 5);
    write(123, 8);
    write(format, 3);
    match format {
        0 => {
            write(width, 8);
            write(height, 8);
        }
        1 => {
            write(width, 16);
            write(height, 16);
        }
        _ => {}
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks_exact(8)
        .map(|group| {
            group
                .iter()
                .fold(0, |value, bit| (value << 1) | u8::from(*bit))
        })
        .collect()
}
#[test]
fn sorenson_explicit_and_standard_picture_sizes_share_one_bit_reader() {
    for (format, width, height) in [
        (0, 52, 38),
        (1, 1920, 1080),
        (2, 352, 288),
        (3, 176, 144),
        (4, 128, 96),
        (5, 320, 240),
        (6, 160, 120),
    ] {
        let bytes = sorenson(format, width, height);
        assert_eq!(dimensions(2, &bytes), Some((width, height)));
        for end in 0..4 {
            assert!(dimensions(2, &bytes[..end]).is_none());
        }
    }
}
#[test]
fn screen_video_versions_use_twelve_bit_dimensions_without_block_size_bits() {
    for codec in [3, 6] {
        let bytes = [
            (0xf000u16 | 52).to_be_bytes(),
            (0x7000u16 | 38).to_be_bytes(),
        ]
        .concat();
        assert_eq!(dimensions(codec, &bytes), Some((52, 38)));
        for end in 0..4 {
            assert!(dimensions(codec, &bytes[..end]).is_none());
        }
    }
}
#[test]
fn unsupported_invalid_and_zero_dimensions_are_rejected() {
    assert!(dimensions(2, &sorenson(7, 0, 0)).is_none());
    assert!(dimensions(2, &sorenson(0, 0, 16)).is_none());
    assert!(dimensions(3, &[0; 4]).is_none());
    assert!(dimensions(4, &[0; 32]).is_none());
    let mut wrong_start = sorenson(0, 16, 16);
    wrong_start[0] = 255;
    assert!(dimensions(2, &wrong_start).is_none());
}
