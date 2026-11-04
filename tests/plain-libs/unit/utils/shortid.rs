use super::*;

#[test]
fn encoding_preserves_every_bit_of_the_random_input() {
    let decode = |id: &str| {
        id.bytes().fold(0u128, |value, byte| {
            value * 57 + ALPHABET.iter().position(|&digit| digit == byte).unwrap() as u128
        })
    };
    for value in std::iter::once(0)
        .chain((0..128).map(|bit| 1u128 << bit))
        .chain(std::iter::once(u128::MAX))
    {
        let id = encode(&value.to_be_bytes());
        assert_eq!(id.len(), 22);
        assert_eq!(decode(&id), value);
    }
}

#[test]
fn length_is_22() {
    assert_eq!(new_id().len(), 22);
}

#[test]
fn unique() {
    let a = new_id();
    let b = new_id();
    assert_ne!(a, b);
}

#[test]
fn no_ambiguous_chars() {
    for _ in 0..50 {
        let id = new_id();
        for c in id.chars() {
            assert!(
                !matches!(c, '0' | 'O' | '1' | 'I' | 'l'),
                "ambiguous char in {id}"
            );
        }
    }
}
