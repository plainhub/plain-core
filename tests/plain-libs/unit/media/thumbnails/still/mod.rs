use super::*;

#[test]
fn classification_respects_the_declared_ftyp_extent() {
    for wide in [false, true] {
        let header = if wide { 16 } else { 8 };
        let size = header + 12;
        let mut bytes = if wide {
            let mut b = 1_u32.to_be_bytes().to_vec();
            b.extend_from_slice(b"ftyp");
            b.extend_from_slice(&(size as u64).to_be_bytes());
            b
        } else {
            let mut b = (size as u32).to_be_bytes().to_vec();
            b.extend_from_slice(b"ftyp");
            b
        };
        bytes.extend_from_slice(b"isom\0\0\0\0isom");
        bytes.extend_from_slice(&12_u32.to_be_bytes());
        bytes.extend_from_slice(b"freeavif");
        assert_eq!(kind(&bytes), None);
        bytes[header + 8..header + 12].copy_from_slice(b"avif");
        assert_eq!(kind(&bytes), Some(ImageKind::Avif));
        bytes[header + 8..header + 12].copy_from_slice(b"heic");
        assert_eq!(kind(&bytes), Some(ImageKind::Heif));
    }
    assert_eq!(kind(b"\0\0\0\x08ftypavif"), None);
    assert_eq!(kind(b"\0\0\0\x01ftyp\0\0\0\0\0\0\0\x08avif"), None);
}
