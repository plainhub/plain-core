use super::*;

#[test]
fn test_merge() {
    // Let's say we have the word 'hello' and a word-to-id vocab that looks
    // like this: {'h': 0, 'e': 1, 'l': 2, 'o': 3}.
    let mut word = Word::new();
    word.add(0, 1); // 'h'
    word.add(1, 1); // 'e'
    word.add(2, 1); // 'l'
    word.add(2, 1); // 'l'
    word.add(3, 1); // 'o'

    // We're going to perform a merge on the pair ('l', 'l') ~= (2, 2). Let's
    // say that 'll' has the ID of 4 in the updated word-to-id vocab.
    let changes = word.merge(2, 2, 4, usize::MAX);

    // So the word should now look like this:
    assert_eq!(
        word.get_chars(),
        &[
            0u32, // 'h'
            1u32, // 'e'
            4u32, // 'll'
            3u32, // 'o'
        ]
    );

    // The return value `changes` will be used to update the pair counts during
    // training. This merge affects the counts for the pairs
    // ('e', 'l') ~= (1, 2),
    // ('e', 'll') ~= (1, 4),
    // ('l', 'o') ~= (2, 3), and
    // ('ll', 'o') ~= (4, 3).
    // So the changes should reflect that:
    assert_eq!(
        changes,
        &[
            ((1u32, 2u32), -1i32), // count for ('e', 'l') should be decreased by 1.
            ((1u32, 4u32), 1i32),  // count for ('e', 'll') should be increased by 1.
            ((2u32, 3u32), -1i32), // count for ('l', 'o') should be decreased by 1.
            ((4u32, 3u32), 1i32),  // count for ('ll', 'o') should be increased by 1.
        ]
    );
}

#[test]
fn test_merge_max_length() {
    // Let's say we have the word 'hello' and a word-to-id vocab that looks
    // like this: {'h': 0, 'e': 1, 'l': 2, 'o': 3}.
    let mut word = Word::new();
    word.add(0, 1); // 'h'
    word.add(1, 1); // 'e'
    word.add(2, 1); // 'l'
    word.add(2, 1); // 'l'
    word.add(3, 1); // 'o'

    // We're going to perform a merge on the pair ('l', 'l') ~= (2, 2). Let's
    // say that 'll' has the ID of 4 in the updated word-to-id vocab.
    let changes = word.merge(2, 2, 4, 2);
    assert_eq!(
        word.get_chars(),
        &[
            0u32, // 'h'
            1u32, // 'e'
            4u32, // 'll'
            3u32, // 'o'
        ]
    );

    assert_eq!(
        changes,
        &[
            ((1u32, 2u32), -1i32), // count for ('e', 'l') should be decreased by 1.
            // ((1u32, 4u32), 1i32),  Missing since this would be larger than 2
            ((2u32, 3u32), -1i32), // count for ('l', 'o') should be decreased by 1.
                                   // ((4u32, 3u32), 1i32), Missing since this would be larger than 2
        ]
    );
}
