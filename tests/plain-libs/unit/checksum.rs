use super::*;

fn reference_adler(start: u32, bytes: &[u8]) -> u32 {
    let mut a = start & 0xFFFF;
    let mut b = start >> 16;
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[test]
fn shared_adler_preserves_vectors_chunking_and_simd_boundaries() {
    assert_eq!(adler32::adler32(1, b"Wikipedia"), 0x11E60398);
    let data = (0..18000)
        .map(|i| ((i * 29 + i / 13) % 256) as u8)
        .collect::<Vec<_>>();
    for len in [
        0, 1, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 5551, 5552, 5553, 16384, 18000,
    ] {
        let bytes = &data[..len];
        let expected = reference_adler(1, bytes);
        assert_eq!(adler32::adler32(1, bytes), expected, "length {len}");
        for size in [1, 17, 65, 1024, 5552] {
            let mut checksum = Adler32::new();
            for chunk in bytes.chunks(size) {
                checksum.write(chunk);
            }
            assert_eq!(checksum.finish(), expected, "length {len}, chunks {size}");
        }
        let mut copy = vec![0; bytes.len() + 8];
        assert_eq!(adler32::adler32_fold_copy(1, &mut copy, bytes), expected);
        assert_eq!(&copy[..bytes.len()], bytes);
        assert_eq!(&copy[bytes.len()..], &[0; 8]);
    }
}
