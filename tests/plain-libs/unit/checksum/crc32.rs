use super::*;

fn reference_crc(start: u32, bytes: &[u8]) -> u32 {
    let mut value = !start;
    for byte in bytes {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = (value >> 1) ^ (0xEDB88320u32.wrapping_mul(value & 1));
        }
    }
    !value
}

#[test]
fn scalar_and_selected_crc_match_independent_vectors_and_chunk_boundaries() {
    assert_eq!(hash(b"123456789"), 0xCBF43926);
    let input = (0..18001)
        .map(|i| ((i * 31 + i / 7) % 256) as u8)
        .collect::<Vec<_>>();
    for offset in 0..16 {
        for len in [
            0, 1, 7, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 1024, 16384,
        ] {
            let bytes = &input[offset..offset + len];
            let expected = reference_crc(0, bytes);
            let mut baseline = baseline::State::new(0);
            baseline.update(bytes);
            assert_eq!(baseline.finalize(), expected);
            let mut selected = Hasher::new();
            for chunk in bytes.chunks(17) {
                selected.update(chunk);
            }
            assert_eq!(selected.amount(), len as u32);
            assert_eq!(selected.sum(), expected);
            assert_eq!(selected.clone().finalize(), expected);
            selected.reset();
            selected.update(bytes);
            assert_eq!(selected.finalize(), expected);
        }
    }
}

#[test]
fn seeded_crc_and_gzip_length_wrap_keep_their_contract() {
    let mut checksum = Hasher::new_with_initial(0x12345678);
    checksum.amount = u32::MAX - 3;
    checksum.update(b"abcdefgh");
    assert_eq!(checksum.amount(), 4);
    assert_eq!(checksum.finalize(), reference_crc(0x12345678, b"abcdefgh"));
}

#[test]
fn constructor_uses_the_available_native_engine() {
    assert_eq!(
        matches!(Hasher::new().state, State::Specialized(_)),
        specialized::State::new(0).is_some()
    );
    #[cfg(target_arch = "aarch64")]
    assert_eq!(
        specialized::State::new(0).is_some(),
        std::arch::is_aarch64_feature_detected!("crc")
    );
}
