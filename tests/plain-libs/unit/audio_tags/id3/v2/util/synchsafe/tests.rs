const UNSYNCHRONIZED_CONTENT: &[u8] = &[0xFF, 0x00, 0x00, 0xFF, 0x12, 0xB0, 0x05, 0xFF, 0x00, 0x00];
const EXPECTED: &[u8] = &[0xFF, 0x00, 0xFF, 0x12, 0xB0, 0x05, 0xFF, 0x00];

#[test]
fn unsynchronized_stream() {
    let reader = Cursor::new(UNSYNCHRONIZED_CONTENT);
    let mut unsynchronized_reader = UnsynchronizedStream::new(reader);

    let mut final_content = Vec::new();
    unsynchronized_reader
        .read_to_end(&mut final_content)
        .unwrap();

    assert_eq!(final_content, EXPECTED);
}

#[test]
fn unsynchronized_stream_large() {
    // Create a buffer >10k to force a buffer reset
    let reader = Cursor::new(UNSYNCHRONIZED_CONTENT.repeat(1000));
    let mut unsynchronized_reader = UnsynchronizedStream::new(reader);

    let mut final_content = Vec::new();
    unsynchronized_reader
        .read_to_end(&mut final_content)
        .unwrap();

    // UNSYNCHRONIZED_CONTENT * 1000 should equal EXPECTED * 1000
    assert_eq!(final_content, EXPECTED.repeat(1000));
}

#[test]
fn unsynchronized_stream_should_not_replace_unrelated() {
    const ORIGINAL_CONTENT: &[u8] = &[0xFF, 0x1A, 0xFF, 0xC0, 0x10, 0x01];

    let reader = Cursor::new(ORIGINAL_CONTENT);
    let mut unsynchronized_reader = UnsynchronizedStream::new(reader);

    let mut final_content = Vec::new();
    unsynchronized_reader
        .read_to_end(&mut final_content)
        .unwrap();

    assert_eq!(final_content, ORIGINAL_CONTENT);
}

use crate::audio_tags::id3::v2::util::synchsafe::{SynchsafeInteger, UnsynchronizedStream};
use std::io::{Cursor, Read};
macro_rules! synchsafe_integer_tests {
    (
        $($int:ident => {
            synch: $original:literal, $new:literal;
            unsynch: $original_unsync:literal, $new_unsynch:literal;
            widen: $original_widen:literal, $new_widen:literal;
        });+
    ) => {
        $(
            mod $int {
                use super::SynchsafeInteger;

                #[test]
                fn synch() {
                    assert_eq!($original.synch().unwrap(), $new);
                }

                #[test]
                fn unsynch() {
                    assert_eq!($original_unsync.unsynch(), $new_unsynch);
                }

                #[test]
                fn widen() {
                    assert_eq!($original_widen.widening_synch(), $new_widen);
                }
            }
        )+
    };
}

synchsafe_integer_tests! {
    u8 => {
        synch:   0x7F_u8, 0x7F_u8;
        unsynch: 0x7F_u8, 0x7F_u8;
        widen:   0xFF_u8, 0x017F_u16;
    };
    u16 => {
        synch:   0x3FFF_u16, 0x7F7F_u16;
        unsynch: 0x7F7F_u16, 0x3FFF_u16;
        widen:   0xFFFF_u16, 0x0003_7F7F_u32;
    };
    u32 => {
        synch:   0xFFF_FFFF_u32, 0x7F7F_7F7F_u32;
        unsynch: 0x7F7F_7F7F_u32, 0xFFF_FFFF_u32;
        widen:   0xFFFF_FFFF_u32, 0x000F_7F7F_7F7F_u64;
    }
}
