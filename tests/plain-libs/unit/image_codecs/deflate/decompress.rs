use super::*;

// Independently encoded fixed-Huffman zlib: three literals followed by a distance-3 length-258 match.
const OVERLAPPING: &[u8] = &[120, 1, 75, 76, 74, 30, 69, 0, 36, 18, 99, 235];

#[test]
fn overlapping_matches_write_only_the_bytes_reported_by_read() {
    let mut decoder = Decompressor::new();
    let mut output = vec![0xa5; 320];
    let (consumed, written) = decoder.read(OVERLAPPING, &mut output, 0, true).unwrap();
    assert_eq!(consumed, OVERLAPPING.len());
    assert_eq!(written, 261);
    assert!(decoder.is_done());
    assert_eq!(&output[..written], b"abc".repeat(87));
    assert!(output[written..].iter().all(|&byte| byte == 0xa5));
}

#[test]
fn exact_copy_preserves_overlapping_patterns_and_untouched_storage() {
    for distance in [1, 2, 3, 7, 15, 16, 17, 31, 128, 258, 32768] {
        for length in [0, 1, 2, 3, 15, 16, 17, 127, 257, 258] {
            let start = distance + 3;
            let mut bytes: Vec<_> = (0..start).map(|n| (n * 37 + 11) as u8).collect();
            bytes.resize(start + length + 19, 0xa5);
            let mut expected = bytes.clone();
            for i in 0..length {
                expected[start + i] = expected[start + i - distance];
            }
            copy_backreference(&mut bytes, start, distance, length);
            assert_eq!(bytes, expected, "distance {distance}, length {length}");
        }
    }
}

const STREAMS: &[(&[u8], &[u8])] = &[
    (&[120, 1, 75, 28, 5, 0, 217, 168, 98, 36], b"a"),
    (&[120, 1, 75, 76, 26, 133, 0, 126, 64, 99, 7], b"ab"),
    (&[120, 1, 75, 76, 74, 30, 69, 0, 36, 18, 99, 235], b"abc"),
    (
        &[
            120, 1, 75, 76, 74, 78, 73, 77, 75, 207, 200, 204, 202, 206, 201, 205, 203, 47, 24,
            229, 3, 0, 0, 250, 111, 204,
        ],
        b"abcdefghijklmnop",
    ),
];

#[test]
fn streaming_matches_keep_pending_copies_and_checksum_validation() {
    for &(stream, seed) in STREAMS {
        let expected: Vec<_> = seed
            .iter()
            .copied()
            .cycle()
            .take(seed.len() + 258)
            .collect();
        for step in [1, 2, 3, 7, 16, 31, 128, 260] {
            let mut decoder = Decompressor::new();
            let mut output = vec![0xa5; expected.len() + 19];
            let (mut consumed, mut written) = (0, 0);
            while !decoder.is_done() {
                let end = (written + step).min(output.len());
                let (input, count) = decoder
                    .read(&stream[consumed..], &mut output[..end], written, true)
                    .unwrap();
                assert!(input != 0 || count != 0 || decoder.is_done());
                consumed += input;
                written += count;
            }
            assert_eq!(written, expected.len());
            assert_eq!(consumed, stream.len());
            assert_eq!(&output[..written], expected);
            assert!(output[written..].iter().all(|&byte| byte == 0xa5));
        }
        let mut bad = stream.to_vec();
        *bad.last_mut().unwrap() ^= 1;
        assert!(matches!(
            decompress_to_vec(&bad),
            Err(DecompressionError::WrongChecksum)
        ));
        assert!(matches!(
            decompress_to_vec(&stream[..stream.len() - 1]),
            Err(DecompressionError::InsufficientInput)
        ));
        match decompress_to_vec_bounded(stream, expected.len() - 1) {
            Err(BoundedDecompressionError::OutputTooLarge { partial_output }) => {
                assert_eq!(partial_output, expected[..expected.len() - 1])
            }
            _ => panic!("output limit must preserve the decoded prefix and reject overflow"),
        }
    }
}

#[test]
fn explicitly_ignored_adler_does_not_compute_an_unused_final_checksum() {
    let mut decoder = Decompressor::new();
    decoder.ignore_adler32();
    let mut output = vec![0; 320];
    let mut stream = OVERLAPPING.to_vec();
    *stream.last_mut().unwrap() ^= 1;
    let (_, written) = decoder.read(&stream, &mut output, 0, true).unwrap();
    assert_eq!(written, 261);
    assert_eq!(decoder.checksum.finish(), 1);
    assert_eq!(&output[..written], b"abc".repeat(87));
    assert!(decoder.is_done());
}
