use super::*;
use regex::Regex;
use crate::tokenizer::unicode_categories::is_mark_nonspacing;

#[test]
fn test_len_range_inclusive() {
    let range = Range::Original(3..=7);
    let len = range.len();
    assert_eq!(len, Some(5)); // 7 - 3 + 1 = 5
}

#[test]
fn test_len_range_exclusive() {
    let range = Range::Original(3..7);
    let len = range.len();
    assert_eq!(len, Some(4)); // 7 - 3 = 4
}

#[test]
fn nfd_adds_new_chars() {
    let mut n = NormalizedString::from("élégant");
    n.nfd();
    assert_eq!(
        &n.alignments,
        &[
            (0, 2),
            (0, 2),
            (0, 2),
            (2, 3),
            (3, 5),
            (3, 5),
            (3, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9)
        ]
    );
    assert_eq!(
        n.alignments_original(),
        vec![
            (0, 3),
            (0, 3),
            (3, 4),
            (4, 7),
            (4, 7),
            (7, 8),
            (8, 9),
            (9, 10),
            (10, 11)
        ]
    );
}

#[test]
fn remove_chars_added_by_nfd() {
    let mut n = NormalizedString::from("élégant");
    n.nfd().filter(|c| !is_mark_nonspacing(c));

    assert_eq!(n.get(), "elegant");

    assert_eq!(
        &n.alignments,
        &[(0, 2), (2, 3), (3, 5), (5, 6), (6, 7), (7, 8), (8, 9)]
    );
    assert_eq!(
        n.alignments_original(),
        vec![
            (0, 1),
            (0, 1),
            (1, 2),
            (2, 3),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7)
        ]
    );
}

#[test]
fn remove_chars() {
    let mut n = NormalizedString::from("élégant");
    n.filter(|c| c != 'n');
    assert_eq!(n.get(), "élégat");
    assert_eq!(
        &n.alignments,
        &[
            (0, 2),
            (0, 2),
            (2, 3),
            (3, 5),
            (3, 5),
            (5, 6),
            (6, 7),
            // Skipped range
            (8, 9)
        ]
    );
    assert_eq!(
        n.alignments_original(),
        vec![
            (0, 2),
            (0, 2),
            (2, 3),
            (3, 5),
            (3, 5),
            (5, 6),
            (6, 7),
            (7, 7), // Eaten n
            (7, 8)
        ]
    );
}

#[test]
fn mixed_addition_and_removal() {
    let mut n = NormalizedString::from("élégant");
    n.nfd().filter(|c| !is_mark_nonspacing(c) && c != 'n');
    assert_eq!(n.get(), "elegat");
    assert_eq!(
        &n.alignments,
        &[(0, 2), (2, 3), (3, 5), (5, 6), (6, 7), (8, 9)]
    );
    assert_eq!(
        n.alignments_original(),
        vec![
            (0, 1),
            (0, 1),
            (1, 2),
            (2, 3),
            (2, 3),
            (3, 4), // g
            (4, 5), // a
            (5, 5), // Eaten n
            (5, 6)
        ]
    );
}

#[test]
fn range_conversion() {
    let mut n = NormalizedString::from("    __Hello__   ");
    n.filter(|c| !c.is_whitespace()).lowercase();
    let hello_n = n.convert_offsets(Range::Original(6..11));
    assert_eq!(hello_n, Some(2..7));
    assert_eq!(
        n.get_range(Range::Normalized(hello_n.clone().unwrap())),
        Some("hello")
    );
    assert_eq!(
        n.get_range_original(Range::Normalized(hello_n.unwrap())),
        Some("Hello")
    );
    assert_eq!(n.get_range(Range::Original(6..11)), Some("hello"));
    assert_eq!(n.get_range_original(Range::Original(6..11)), Some("Hello"));

    // Make sure we get None only in specific cases
    assert_eq!(n.convert_offsets(Range::Original(0..0)), Some(0..0));
    assert_eq!(n.convert_offsets(Range::Original(3..3)), Some(3..3));
    assert_eq!(n.convert_offsets(Range::Original(15..)), Some(9..9));
    assert_eq!(n.convert_offsets(Range::Original(16..)), Some(16..16));
    assert_eq!(n.convert_offsets(Range::Original(17..)), None);
    assert_eq!(n.convert_offsets(Range::Normalized(0..0)), Some(0..0));
    assert_eq!(n.convert_offsets(Range::Normalized(3..3)), Some(3..3));
    assert_eq!(n.convert_offsets(Range::Normalized(9..)), Some(9..9));
    assert_eq!(n.convert_offsets(Range::Normalized(10..)), None);
}

#[test]
fn original_range() {
    let mut n = NormalizedString::from("Hello_______ World!");
    n.filter(|c| c != '_').lowercase();
    let world_n = n.get_range(Range::Normalized(6..11)).unwrap();
    let world_o = n.get_range_original(Range::Normalized(6..11)).unwrap();
    assert_eq!(world_n, "world");
    assert_eq!(world_o, "World");
    let original_range = Range::Original(n.convert_offsets(Range::Normalized(6..11)).unwrap());
    assert_eq!(n.get_range(original_range.clone()).unwrap(), "world");
    assert_eq!(
        n.get_range_original(original_range.clone()).unwrap(),
        "World"
    );
    assert_eq!(original_range.into_full_range(n.len_original()), 13..18);
}

#[test]
fn added_around_edges() {
    let mut n = NormalizedString::from("Hello");
    n.transform(
        vec![
            (' ', 1),
            ('H', 0),
            ('e', 0),
            ('l', 0),
            ('l', 0),
            ('o', 0),
            (' ', 1),
        ],
        0,
    );

    assert_eq!(&n.normalized, " Hello ");
    assert_eq!(
        n.get_range_original(Range::Normalized(1..n.normalized.len() - 1)),
        Some("Hello")
    );
}

#[test]
fn added_characters_alignment() {
    let mut n = NormalizedString::from("野口 No");
    n.transform(
        n.get().to_owned().chars().flat_map(|c| {
            if (c as usize) > 0x4E00 {
                vec![(' ', 0), (c, 1), (' ', 1)]
            } else {
                vec![(c, 0)]
            }
        }),
        0,
    );

    assert_eq!(
        n,
        NormalizedString {
            original: "野口 No".into(),
            normalized: " 野  口  No".into(),
            alignments: vec![
                (0, 3),
                (0, 3),
                (0, 3),
                (0, 3),
                (0, 3),
                (3, 6),
                (3, 6),
                (3, 6),
                (3, 6),
                (3, 6),
                (6, 7),
                (7, 8),
                (8, 9)
            ],
            original_shift: 0
        }
    );
    assert_eq!(
        n.alignments_original(),
        vec![
            (0, 5),
            (0, 5),
            (0, 5),
            (5, 10),
            (5, 10),
            (5, 10),
            (10, 11),
            (11, 12),
            (12, 13)
        ]
    );
}

#[test]
fn remove_at_beginning() {
    let mut n = NormalizedString::from("     Hello");
    n.filter(|c| !c.is_whitespace());
    assert_eq!(
        n.get_range_original(Range::Normalized(1.."Hello".len())),
        Some("ello")
    );
    assert_eq!(
        n.get_range_original(Range::Normalized(0..n.normalized.len())),
        Some("Hello")
    );
}

#[test]
fn remove_at_end() {
    let mut n = NormalizedString::from("Hello    ");
    n.filter(|c| !c.is_whitespace());
    assert_eq!(n.get_range_original(Range::Normalized(0..4)), Some("Hell"));
    assert_eq!(
        n.get_range_original(Range::Normalized(0..n.normalized.len())),
        Some("Hello")
    );
}

#[test]
fn removed_around_both_edges() {
    let mut n = NormalizedString::from("  Hello  ");
    n.filter(|c| !c.is_whitespace());
    assert_eq!(&n.normalized, "Hello");

    assert_eq!(
        n.get_range_original(Range::Normalized(0.."Hello".len())),
        Some("Hello")
    );
    assert_eq!(
        n.get_range_original(Range::Normalized(1.."Hell".len())),
        Some("ell")
    );
}

#[test]
fn lstrip() {
    let mut n = NormalizedString::from("  This is an example  ");
    n.lstrip();
    assert_eq!(&n.normalized, "This is an example  ");
    assert_eq!(
        n.get_range_original(Range::Normalized(0..n.normalized.len())),
        Some("This is an example  ")
    );
}

#[test]
fn rstrip() {
    let mut n = NormalizedString::from("  This is an example  ");
    n.rstrip();
    assert_eq!(&n.normalized, "  This is an example");
    assert_eq!(
        n.get_range_original(Range::Normalized(0..n.normalized.len())),
        Some("  This is an example")
    );
}

#[test]
fn strip() {
    let mut n = NormalizedString::from("  This is an example  ");
    n.strip();
    assert_eq!(&n.normalized, "This is an example");
    assert_eq!(
        n.get_range_original(Range::Normalized(0..n.normalized.len())),
        Some("This is an example")
    );
}

#[test]
fn strip_unicode() {
    let mut n = NormalizedString::from("  你好asa \n");
    n.strip();
    assert_eq!(&n.normalized, "你好asa");
    assert_eq!(
        n.get_range_original(Range::Normalized(0..n.normalized.len())),
        Some("你好asa")
    );
}

#[test]
fn prepend() {
    let mut n = NormalizedString::from("there");
    n.prepend("Hey ");
    assert_eq!(&n.normalized, "Hey there");
    assert_eq!(
        n.alignments,
        vec![
            (0, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5)
        ]
    );
    assert_eq!(n.convert_offsets(Range::Normalized(0..4)), Some(0..1));
}

#[test]
fn append() {
    let mut n = NormalizedString::from("Hey");
    n.append(" there");
    assert_eq!(&n.normalized, "Hey there");
    assert_eq!(
        n.alignments,
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (2, 3),
            (2, 3),
            (2, 3),
            (2, 3),
            (2, 3),
            (2, 3)
        ]
    );
    assert_eq!(
        n.convert_offsets(Range::Normalized(3.." there".len())),
        Some(2..3)
    );
}

#[test]
fn get_range() {
    let s = String::from("Hello my name is John 👋");
    assert_eq!(get_range_of(&s, ..), Some(&s[..]));
    assert_eq!(get_range_of(&s, 17..), Some("John 👋"));
}

#[test]
fn slice() {
    let mut s = NormalizedString::from("𝔾𝕠𝕠𝕕 𝕞𝕠𝕣𝕟𝕚𝕟𝕘");
    s.nfkc();

    let original_slice = s.slice(Range::Original(0..4)).unwrap();
    assert_eq!(original_slice.get(), "G");
    assert_eq!(original_slice.get_original(), "𝔾");

    let normalized_slice = s.slice(Range::Normalized(0..4)).unwrap();
    assert_eq!(normalized_slice.get(), "Good");
    assert_eq!(normalized_slice.get_original(), "𝔾𝕠𝕠𝕕");

    // Make sure the sliced NormalizedString is still aligned as expected
    let mut s = NormalizedString::from("   Good Morning!   ");
    s.strip();

    // If we keep the whole slice
    let slice = s.slice(Range::Original(..)).unwrap();
    assert_eq!(
        slice.get_range_original(Range::Normalized(0..4)),
        Some("Good")
    );
    let slice = s.slice(Range::Normalized(..)).unwrap();
    assert_eq!(
        slice.get_range_original(Range::Normalized(0..4)),
        Some("Good")
    );

    // If we keep after the modified piece
    let slice = s.slice(Range::Original(4..15)).unwrap();
    assert_eq!(
        slice.get_range_original(Range::Normalized(0..3)),
        Some("ood")
    );

    // If we keep only the modified piece
    let slice = s.slice(Range::Original(3..16)).unwrap();
    assert_eq!(
        slice.get_range_original(Range::Normalized(0..4)),
        Some("Good")
    );
}

#[test]
fn replace() {
    // Simple
    let mut s = NormalizedString::from(" Hello   friend ");
    s.replace(' ', "_").unwrap();
    assert_eq!(s.get(), "_Hello___friend_");
    let mut s = NormalizedString::from("aaaab");
    s.replace('a', "b").unwrap();
    assert_eq!(s.get(), "bbbbb");

    // Overlapping
    let mut s = NormalizedString::from("aaaab");
    s.replace("aaa", "b").unwrap();
    assert_eq!(s.get(), "bab");

    // Regex
    let mut s = NormalizedString::from(" Hello   friend ");
    let re = Regex::new(r"\s+").unwrap();
    s.replace(&re, "_").unwrap();
    assert_eq!(s.get(), "_Hello_friend_");
}

#[test]
fn split() {
    use SplitDelimiterBehavior::*;
    let s = NormalizedString::from("The-final--countdown");

    let test = |behavior: SplitDelimiterBehavior, result: Vec<&str>| {
        let splits = s.split('-', behavior).unwrap();
        assert_eq!(splits.iter().map(|n| n.get()).collect::<Vec<_>>(), result);
    };

    test(Removed, vec!["The", "final", "countdown"]);
    test(Isolated, vec!["The", "-", "final", "-", "-", "countdown"]);
    test(MergedWithPrevious, vec!["The-", "final-", "-", "countdown"]);
    test(MergedWithNext, vec!["The", "-final", "-", "-countdown"]);
    test(Contiguous, vec!["The", "-", "final", "--", "countdown"]);
}

#[test]
fn transform_range_single_bytes() {
    let s = NormalizedString::from("Hello friend");

    // Removing at the beginning
    let mut current = s.clone();
    current.transform_range(Range::Original(0..4), vec![('Y', 0)], 3);
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "Yo friend".into(),
            alignments: vec![
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
                (9, 10),
                (10, 11),
                (11, 12)
            ],
            original_shift: 0,
        }
    );

    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9)
        ]
    );

    // Removing in the middle
    let mut current = s.clone();
    current.transform_range(
        Range::Original(3..10),
        vec![('_', 0), ('F', 0), ('R', -2)],
        2,
    );
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "Hel_FRnd".into(),
            alignments: vec![
                (0, 1),
                (1, 2),
                (2, 3),
                (5, 6),
                (6, 7),
                (7, 8),
                (10, 11),
                (11, 12)
            ],
            original_shift: 0,
        }
    );

    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 3),
            (3, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 6),
            (6, 6),
            (6, 7),
            (7, 8)
        ]
    );

    // Removing at the end
    let mut current = s.clone();
    current.transform_range(Range::Original(5..), vec![('_', 0), ('F', -5)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "Hello_F".into(),
            alignments: vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7)],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 7),
            (7, 7),
            (7, 7),
            (7, 7),
            (7, 7)
        ]
    );

    // Adding at the beginning
    let mut current = s.clone();
    current.transform_range(Range::Original(0..1), vec![('H', 1), ('H', 0)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "HHello friend".into(),
            alignments: vec![
                (0, 0),
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
                (9, 10),
                (10, 11),
                (11, 12)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9),
            (9, 10),
            (10, 11),
            (11, 12),
            (12, 13)
        ]
    );
    // Equivalent to the previous one
    let mut current = s.clone();
    current.transform_range(Range::Original(0..0), vec![('H', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "HHello friend".into(),
            alignments: vec![
                (0, 0),
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
                (9, 10),
                (10, 11),
                (11, 12)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9),
            (9, 10),
            (10, 11),
            (11, 12),
            (12, 13)
        ]
    );
    // Adding as part of the first character
    let mut current = s.clone();
    current.transform_range(Range::Original(0..1), vec![('H', 0), ('H', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "HHello friend".into(),
            alignments: vec![
                (0, 1),
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
                (9, 10),
                (10, 11),
                (11, 12)
            ],
            original_shift: 0,
        }
    );

    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9),
            (9, 10),
            (10, 11),
            (11, 12),
            (12, 13)
        ]
    );

    // Adding in the middle
    let mut current = s.clone();
    current.transform_range(
        Range::Original(5..6),
        vec![('_', 0), ('m', 1), ('y', 1), ('_', 1)],
        0,
    );
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "Hello_my_friend".into(),
            alignments: vec![
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (5, 6),
                (5, 6),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
                (9, 10),
                (10, 11),
                (11, 12)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 9),
            (9, 10),
            (10, 11),
            (11, 12),
            (12, 13),
            (13, 14),
            (14, 15)
        ]
    );

    // Adding at the end
    let mut current = s;
    current.transform_range(Range::Original(11..), vec![('d', 0), ('_', 1), ('!', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "Hello friend".into(),
            normalized: "Hello friend_!".into(),
            alignments: vec![
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
                (9, 10),
                (10, 11),
                (11, 12),
                (11, 12),
                (11, 12)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9),
            (9, 10),
            (10, 11),
            (11, 14)
        ]
    );
}

#[test]
fn transform_range_multiple_bytes() {
    let s = NormalizedString::from("𝔾𝕠𝕠𝕕");

    // Removing at the beginning
    let mut current = s.clone();
    current.transform_range(Range::Original(0..8), vec![('G', -1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "G𝕠𝕕".into(),
            alignments: vec![
                (0, 4),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 5),
            (1, 5),
            (1, 5),
            (1, 5),
            (5, 9),
            (5, 9),
            (5, 9),
            (5, 9)
        ]
    );
    assert_eq!(current.get_range(Range::Original(0..8)).unwrap(), "G");
    assert_eq!(current.get_range(Range::Original(0..4)).unwrap(), "G");
    assert_eq!(
        current.get_range_original(Range::Original(0..4)).unwrap(),
        "𝔾"
    );
    assert_eq!(
        current.get_range_original(Range::Original(0..8)).unwrap(),
        "𝔾𝕠"
    );

    // Removing in the middle
    let mut current = s.clone();
    current.transform_range(Range::Original(4..12), vec![('o', -1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "𝔾o𝕕".into(),
            alignments: vec![
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 4),
            (0, 4),
            (0, 4),
            (0, 4),
            (4, 5),
            (4, 5),
            (4, 5),
            (4, 5),
            (5, 5),
            (5, 5),
            (5, 5),
            (5, 5),
            (5, 9),
            (5, 9),
            (5, 9),
            (5, 9)
        ]
    );

    // Removing at the end
    let mut current = s.clone();
    current.transform_range(Range::Original(12..), vec![('d', 0), ('!', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "𝔾𝕠𝕠d!".into(),
            alignments: vec![
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );

    // Adding at the beginning
    let mut current = s.clone();
    current.transform_range(Range::Original(0..4), vec![('_', 1), ('𝔾', 0)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "_𝔾𝕠𝕠𝕕".into(),
            alignments: vec![
                (0, 0),
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (1, 5),
            (1, 5),
            (1, 5),
            (1, 5),
            (5, 9),
            (5, 9),
            (5, 9),
            (5, 9),
            (9, 13),
            (9, 13),
            (9, 13),
            (9, 13),
            (13, 17),
            (13, 17),
            (13, 17),
            (13, 17)
        ]
    );

    assert_eq!(current.get_range(Range::Original(0..8)).unwrap(), "𝔾𝕠");
    assert_eq!(current.get_range(Range::Original(0..4)).unwrap(), "𝔾");
    assert_eq!(
        current.get_range_original(Range::Original(0..4)).unwrap(),
        "𝔾"
    );
    assert_eq!(
        current.get_range_original(Range::Original(0..8)).unwrap(),
        "𝔾𝕠"
    );
    // Equivalent to the previous one
    let mut current = s.clone();
    current.transform_range(Range::Original(0..0), vec![('_', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "_𝔾𝕠𝕠𝕕".into(),
            alignments: vec![
                (0, 0),
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (1, 5),
            (1, 5),
            (1, 5),
            (1, 5),
            (5, 9),
            (5, 9),
            (5, 9),
            (5, 9),
            (9, 13),
            (9, 13),
            (9, 13),
            (9, 13),
            (13, 17),
            (13, 17),
            (13, 17),
            (13, 17)
        ]
    );

    assert_eq!(current.get_range(Range::Original(0..8)).unwrap(), "𝔾𝕠");
    assert_eq!(current.get_range(Range::Original(0..4)).unwrap(), "𝔾");
    assert_eq!(
        current.get_range_original(Range::Original(0..4)).unwrap(),
        "𝔾"
    );
    assert_eq!(
        current.get_range_original(Range::Original(0..8)).unwrap(),
        "𝔾𝕠"
    );
    // Adding as part of the first character
    let mut current = s.clone();
    current.transform_range(Range::Original(0..4), vec![('𝔾', 0), ('o', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "𝔾o𝕠𝕠𝕕".into(),
            alignments: vec![
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 5),
            (0, 5),
            (0, 5),
            (0, 5),
            (5, 9),
            (5, 9),
            (5, 9),
            (5, 9),
            (9, 13),
            (9, 13),
            (9, 13),
            (9, 13),
            (13, 17),
            (13, 17),
            (13, 17),
            (13, 17)
        ]
    );
    assert_eq!(current.get_range(Range::Original(0..8)).unwrap(), "𝔾o𝕠");
    assert_eq!(current.get_range(Range::Original(0..4)).unwrap(), "𝔾o");
    assert_eq!(
        current.get_range_original(Range::Original(0..4)).unwrap(),
        "𝔾"
    );
    assert_eq!(
        current.get_range_original(Range::Original(0..8)).unwrap(),
        "𝔾𝕠"
    );

    // Adding in the middle
    let mut current = s.clone();
    current.transform_range(
        Range::Original(4..8),
        vec![('𝕠', 0), ('o', 1), ('o', 1), ('o', 1)],
        0,
    );
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "𝔾𝕠ooo𝕠𝕕".into(),
            alignments: vec![
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 4),
            (0, 4),
            (0, 4),
            (0, 4),
            (4, 11),
            (4, 11),
            (4, 11),
            (4, 11),
            (11, 15),
            (11, 15),
            (11, 15),
            (11, 15),
            (15, 19),
            (15, 19),
            (15, 19),
            (15, 19)
        ]
    );

    // Adding at the end
    let mut current = s;
    current.transform_range(Range::Original(16..), vec![('!', 1)], 0);
    assert_eq!(
        current,
        NormalizedString {
            original: "𝔾𝕠𝕠𝕕".into(),
            normalized: "𝔾𝕠𝕠𝕕!".into(),
            alignments: vec![
                (0, 4),
                (0, 4),
                (0, 4),
                (0, 4),
                (4, 8),
                (4, 8),
                (4, 8),
                (4, 8),
                (8, 12),
                (8, 12),
                (8, 12),
                (8, 12),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16),
                (12, 16)
            ],
            original_shift: 0,
        }
    );
    assert_eq!(
        current.alignments_original(),
        vec![
            (0, 4),
            (0, 4),
            (0, 4),
            (0, 4),
            (4, 8),
            (4, 8),
            (4, 8),
            (4, 8),
            (8, 12),
            (8, 12),
            (8, 12),
            (8, 12),
            (12, 17),
            (12, 17),
            (12, 17),
            (12, 17)
        ]
    );
}

#[test]
fn transform_check() {
    let mut s = NormalizedString::from("abc…");
    s.nfkd();
    let transforms = vec![('a', -2), ('.', 0), ('.', 0), ('.', 0)];
    s.transform(transforms, 0);
    s.lowercase();
    assert_eq!(s.get(), "a...");
}

#[test]
fn test_append_after_clear() {
    let mut n = NormalizedString::from("Hello");
    assert_eq!(n.get(), "Hello");

    n.clear();
    assert_eq!(n.get(), "");

    n.append(" World");
    assert_eq!(n.get(), " World");

    assert_eq!(n.len_original(), 5);
    assert_eq!(n.len(), 6);

    assert_eq!(n.get_range_original(Range::Original(0..5)), Some("Hello"));
    assert_eq!(n.get_range_original(Range::Normalized(0..6)), Some(""));

    assert_eq!(n.get_range(Range::Normalized(0..6)), Some(" World"));
}
