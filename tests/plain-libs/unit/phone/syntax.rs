use super::*;

#[test]
fn every_bundled_unicode_decimal_alphabet_normalizes_to_ascii_digits() {
    for zero in &PLANS.decimal_zeroes {
        for value in 0..10 {
            assert_eq!(
                digit(char::from_u32(zero + value).unwrap()),
                Some(char::from(b'0' + value as u8))
            );
        }
    }
    for value in ['x', 'Ⅶ', '²', '①', '\0'] {
        assert_eq!(digit(value), None);
    }
}

#[test]
fn extensions_contexts_and_display_prefixes_keep_the_same_global_number() {
    let number = crate::phone::tests::example("CN", crate::phone::number::NumberType::Mobile);
    let canonical = prepare(&number).unwrap().digits;
    for value in [
        format!("Call {number}"),
        format!("tel:{number};ext=42"),
        format!("{number} ext. 42"),
        format!("{number} x42"),
        format!("{number},,42"),
    ] {
        let value = prepare(&value).unwrap();
        assert!(value.plus);
        assert_eq!(value.digits, canonical);
    }
    for value in [
        format!("tel:{};phone-context=+86", &canonical[2..]),
        format!("tel:{};phone-context=example.invalid", &canonical[2..]),
    ] {
        assert!(prepare(&value).is_some());
    }
    assert!(prepare("tel:123;phone-context=+").is_none());
    assert!(prepare("tel:123;phone-context=example..invalid").is_none());
    assert!(prepare(&"1".repeat(251)).is_none());
}

#[test]
fn unicode_digits_remain_present_when_ascii_letters_form_a_vanity_number() {
    let number = crate::phone::tests::example("CN", crate::phone::number::NumberType::Mobile);
    let vanity = number[..6].to_owned()
        + &number[6..]
            .chars()
            .map(|digit| match digit {
                '2' => 'A',
                '3' => 'D',
                '4' => 'G',
                '5' => 'J',
                '6' => 'M',
                '7' => 'P',
                '8' => 'T',
                '9' => 'W',
                value => value,
            })
            .collect::<String>();
    assert!(vanity.bytes().filter(u8::is_ascii_alphabetic).count() >= 3);
    for zero in [0x660, 0xff10, 0x1d7ce] {
        let input = vanity
            .chars()
            .map(|ch| {
                if ch.is_ascii_digit() {
                    char::from_u32(zero + u32::from(ch) - u32::from('0')).unwrap()
                } else {
                    ch
                }
            })
            .collect::<String>();
        assert_eq!(prepare(&input).unwrap().digits, &number[1..]);
    }
}
