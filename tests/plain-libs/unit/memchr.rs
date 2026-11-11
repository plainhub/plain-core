use super::*;
#[test]
fn vector_and_scalar_search_agree_at_every_alignment_and_boundary() {
    for offset in 0..64 {
        let mut storage = vec![17; offset + 513];
        for index in 0..513 {
            storage[offset + index] = ((index * 97 + 13) % 251) as u8;
        }
        let bytes = &storage[offset..];
        for needle in 0..=255 {
            assert_eq!(
                memchr(needle, bytes),
                bytes.iter().position(|x| *x == needle)
            );
            assert_eq!(
                memrchr(needle, bytes),
                bytes.iter().rposition(|x| *x == needle)
            );
        }
        for size in [1, 2, 3, 15, 16, 31, 32, 63, 64, 127] {
            let needle = &bytes[131..131 + size];
            assert_eq!(
                memmem::find(bytes, needle),
                bytes.windows(size).position(|x| x == needle)
            );
            assert_eq!(
                memmem::rfind(bytes, needle),
                bytes.windows(size).rposition(|x| x == needle)
            );
        }
    }
    assert_eq!(memmem::find(b"", b""), Some(0));
}
