use super::{ByteReader, Color, codes};
use std::convert::Infallible;

#[test]
fn test_bits() {
    let mut bits = ByteReader::new(
        [0b0000_1101, 0b1010_0000]
            .into_iter()
            .map(Ok::<_, Infallible>),
    )
    .unwrap();
    assert_eq!(codes::run(Color::Black, &mut bits), Some(42));
}

#[test]
fn lookup_refuses_reader_errors_after_a_matching_code() {
    let mut bits =
        ByteReader::new([Ok(0b0000_1101), Ok(0b1100_0000), Err(())].into_iter()).unwrap();
    assert_eq!(codes::run(Color::Black, &mut bits), None);
}
