use super::*;
use std::io::Cursor;

#[test]
fn eof_metadata_reads_consume_at_most_the_limit_plus_one_byte() {
    for (limit, size, prefix) in [(0, 0, 0), (0, 1, 0), (16, 16, 0), (16, 17, 0), (16, 64, 8)] {
        let mut input = Cursor::new(vec![1; size]);
        let mut output = vec![2; prefix];
        let result = read_to_end(&mut input, &mut output, limit);
        let consumed = size.min(limit - prefix + 1);
        assert_eq!(input.position() as usize, consumed);
        assert_eq!(output.len(), prefix + consumed);
        assert_eq!(result.is_ok(), prefix + size <= limit);
        if let Err(error) = result {
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }
}

#[test]
fn oversized_exact_metadata_reads_fail_before_consuming_or_allocating() {
    let mut input = Cursor::new([1; 16]);
    let mut output = Vec::new();
    let error = input
        .read_exact_len(&mut output, super::super::MAX_METADATA_BYTES + 1)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(input.position(), 0);
    assert!(output.is_empty());
}
