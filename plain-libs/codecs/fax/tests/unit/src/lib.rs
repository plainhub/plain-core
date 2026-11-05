use super::*;

#[test]
fn test_bits() {
    let mut bits = slice_reader(&[0b0000_1101, 0b1010_0000]);
    assert_eq!(maps::black::decode(&mut bits), Some(42));
}

#[test]
fn lookup_refuses_reader_errors_after_a_matching_code() {
    struct FailedRead;
    impl BitReader for FailedRead {
        type Error = ();
        fn peek(&self, bits: u8) -> Option<u16> {
            Some(((0b0000110111_u32 << 6) >> (16 - bits)) as u16)
        }
        fn consume(&mut self, _: u8) -> Result<(), ()> {
            Err(())
        }
        fn bits_to_byte_boundary(&self) -> u8 {
            0
        }
    }
    assert_eq!(maps::black::decode(&mut FailedRead), None);
}
