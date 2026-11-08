use super::*;
use std::mem::{offset_of, size_of};

// The offsets are the slots in the Windows Media Foundation interfaces, including inherited methods.
const _: () = {
    const SLOT: usize = size_of::<usize>();
    assert!(size_of::<Guid>() == 16);
    assert!(offset_of!(Unknown, release) == 2 * SLOT);
    assert!(offset_of!(Attributes, get_u32) == 7 * SLOT);
    assert!(offset_of!(Attributes, get_u64) == 8 * SLOT);
    assert!(offset_of!(Attributes, get_guid) == 10 * SLOT);
    assert!(offset_of!(Attributes, set_u32) == 21 * SLOT);
    assert!(offset_of!(Attributes, set_u64) == 22 * SLOT);
    assert!(offset_of!(Attributes, set_guid) == 24 * SLOT);
    assert!(offset_of!(Reader, select) == 4 * SLOT);
    assert!(offset_of!(Reader, native_media_type) == 5 * SLOT);
    assert!(offset_of!(Reader, current_media_type) == 6 * SLOT);
    assert!(offset_of!(Reader, set_media_type) == 7 * SLOT);
    assert!(offset_of!(Reader, read_sample) == 9 * SLOT);
    assert!(offset_of!(Reader, stream_service) == 11 * SLOT);
    assert!(offset_of!(MediaSource, descriptor) == 8 * SLOT);
    assert!(offset_of!(Descriptor, stream_count) == 33 * SLOT);
    assert!(offset_of!(Writer, add_stream) == 3 * SLOT);
    assert!(offset_of!(Writer, set_input) == 4 * SLOT);
    assert!(offset_of!(Writer, begin) == 5 * SLOT);
    assert!(offset_of!(Writer, write_sample) == 6 * SLOT);
    assert!(offset_of!(Writer, send_tick) == 7 * SLOT);
    assert!(offset_of!(Writer, finish) == 11 * SLOT);
};

#[test]
fn media_source_guid_matches_native_byte_layout() {
    let guid = MEDIA_SOURCE_IID;
    assert_eq!(guid.data1, 0x279a808d);
    assert_eq!(guid.data2, 0xaec7);
    assert_eq!(guid.data3, 0x40c8);
    assert_eq!(guid.data4, [0x9c, 0x6b, 0xa6, 0xb4, 0x92, 0xc7, 0x8a, 0x66]);
}
