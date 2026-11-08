use super::*;
use std::io::Cursor;

#[test]
fn truncated_android_index_is_rejected() {
    let mut bytes = b"tzdata2026a\0".to_vec();
    bytes.extend(24_u32.to_be_bytes());
    bytes.extend(25_u32.to_be_bytes());
    bytes.extend(25_u32.to_be_bytes());
    bytes.push(0);
    assert_eq!(
        find_tz_data::<ANDROID_ENTRY_LEN>(Cursor::new(bytes), b"Etc/UTC")
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );
}
