use super::hash;

#[test]
fn matches_official_c_reference_for_every_tail_and_stripe_boundary() {
    let expected = include_bytes!("../../../testdata/xxh64/reference.bin");
    let lengths: Vec<usize> = (0..=1024)
        .chain([2047, 2048, 2049, 4095, 4096, 4097, 65535, 65536, 65537])
        .collect();
    assert_eq!(expected.len(), 4 * lengths.len() * 8);
    let mut hashes = expected.chunks_exact(8);
    for pattern in 0..4 {
        for &length in &lengths {
            let bytes: Vec<u8> = (0..length)
                .map(|index| match pattern {
                    0 => 0,
                    1 => 255,
                    2 => index as u8,
                    _ => (index.wrapping_mul(131).wrapping_add(17)) as u8,
                })
                .collect();
            let expected = u64::from_le_bytes(hashes.next().unwrap().try_into().unwrap());
            assert_eq!(hash(&bytes), expected, "pattern={pattern} length={length}");
            for offset in 1..=8 {
                let mut unaligned = vec![0; offset];
                unaligned.extend_from_slice(&bytes);
                assert_eq!(hash(&unaligned[offset..]), expected);
            }
        }
    }
}

#[test]
fn standard_vectors_are_stable() {
    assert_eq!(hash(b""), 0xef46db3751d8e999);
    assert_eq!(hash(b"a"), 0xd24ec4f1a98c6e5b);
    assert_eq!(hash(b"abc"), 0x44bc2cf5ad770999);
}
