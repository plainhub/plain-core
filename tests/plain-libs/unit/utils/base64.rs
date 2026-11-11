use super::*;

#[test]
fn rfc4648_vectors() {
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64_encode(plain.as_bytes()), encoded, "encode {plain:?}");
        assert_eq!(
            base64_decode(encoded),
            plain.as_bytes(),
            "decode {encoded:?}"
        );
    }
    // The URL-safe alphabet swaps the two characters that are unsafe in
    // a URL, and decodes to exactly the same bytes.
    assert_eq!(base64_encode_url_safe(&[0xfb, 0xff, 0xbf]), "-_-_");
    assert_eq!(base64_decode("-_-_"), vec![0xfb, 0xff, 0xbf]);
}

#[test]
fn binary_round_trip_over_every_byte() {
    let all: Vec<u8> = (0..=255u8).collect();
    for encoded in [
        base64_encode(&all),
        base64_encode_url_safe(&all),
        base64_encode(&all).replace('=', ""),
    ] {
        assert_eq!(base64_decode(&encoded), all);
    }
}

#[test]
fn unpadded_input_decodes_like_padded_input() {
    assert_eq!(base64_decode("Zm9vYg"), b"foob".to_vec());
    assert_eq!(base64_decode("Zg"), b"f".to_vec());
}

#[test]
fn junk_is_rejected_instead_of_decoding_to_garbage() {
    // The old decoder mapped every unknown character to 0, so a
    // corrupted key still "decoded" to a full-length buffer and passed
    // length checks downstream.
    for junk in ["Zm9v!!!!", "Zm9v Yg==", "Zg==Zg", "@@@@", "A"] {
        assert!(
            base64_decode_checked(junk).is_err(),
            "{junk:?} must be rejected"
        );
    }
    assert!(base64_decode_checked("Zm9v!!!!").is_err());
    // Mixed alphabets in one string are a mistake, not a URL-safe value.
    assert!(base64_decode_checked("ab-_cd+/").is_err());
    // Report where it went wrong.
    assert_eq!(
        base64_decode_checked("Zm9v!"),
        Err(Invalid {
            offset: 4,
            found: '!'
        })
    );
}

#[test]
fn standard_decoding_preserves_padding_alphabet_and_unused_bit_checks() {
    for (text, expected) in [("", ""), ("Zg==", "f"), ("Zm8=", "fo"), ("Zm9v", "foo")] {
        assert_eq!(
            base64_decode_standard(text.as_bytes()).unwrap(),
            expected.as_bytes()
        );
    }
    for text in [
        "Zg", "Zg=", "Zg===", "Zh==", "Zm9=", "____", "Zm 9v", "Zm=v",
    ] {
        assert!(base64_decode_standard(text.as_bytes()).is_err(), "{text}");
    }
    assert_eq!(base64_decode_checked("Zg").unwrap(), b"f");
}

#[test]
fn complete_blocks_and_tail_use_the_requested_alphabet() {
    let mut bytes = [0xfb, 0xff, 0xbf].repeat(64);
    bytes.extend_from_slice(&[0xfb, 0xff]);
    assert_eq!(base64_encode(&bytes), "+/+/".repeat(64) + "+/8=");
    assert_eq!(base64_encode_url_safe(&bytes), "-_-_".repeat(64) + "-_8=");
}

#[test]
fn segmented_encoding_matches_contiguous_bytes_for_all_boundaries() {
    for length in 0..130 {
        let bytes: Vec<_> = (0..length).map(|n| (n * 97) as u8).collect();
        let expected = base64_encode(&bytes);
        for split in 0..=length {
            let mut output = Vec::with_capacity(7 + expected.len());
            output.extend_from_slice(b"prefix:");
            let pointer = output.as_ptr();
            base64_encode_append_segments(&mut output, &[&bytes[..split], b"", &bytes[split..]]);
            assert_eq!(&output[7..], expected.as_bytes());
            assert_eq!(output.as_ptr(), pointer);
        }
    }
}

#[test]
fn strict_alphabet_tail_bits_and_error_offsets_survive_block_boundaries() {
    for blocks in 0..64 {
        let prefix = "AAAA".repeat(blocks);
        let valid = prefix.clone() + "AA==";
        assert_eq!(
            base64_decode_standard(valid.as_bytes()).unwrap(),
            vec![0; blocks * 3 + 1]
        );
        for (tail, offset, found) in [
            ("AA+_", 3, '_'),
            ("AA_+", 3, '+'),
            ("AAA+AAA_", 7, '_'),
            ("AAA_AAA+", 7, '+'),
            ("AA🧪", 2, '🧪'),
        ] {
            let input = prefix.clone() + tail;
            let error = base64_decode_checked(&input).unwrap_err();
            assert_eq!((error.offset, error.found), (blocks * 4 + offset, found));
        }
        for tail in ["AAB=", "AB=="] {
            let input = prefix.clone() + tail;
            assert!(base64_decode_standard(input.as_bytes()).is_err());
        }
        for tail in ["AA-_", "AAA_"] {
            let input = prefix.clone() + tail;
            assert!(base64_decode_standard(input.as_bytes()).is_err());
        }
    }
}
