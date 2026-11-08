use super::*;

#[test]
fn test_populate_nodes_unk() {
    let pieces = vec![("<unk>".to_string(), 0.0)];
    let model = Unigram::from(pieces, Some(0), false).unwrap();

    let mut lattice = Lattice::from("abc", model.bos_id, model.eos_id);
    model.populate_nodes(&mut lattice);

    assert_eq!(lattice.begin_nodes[0].len(), 1);
    assert_eq!(lattice.begin_nodes[1].len(), 1);
    assert_eq!(lattice.begin_nodes[2].len(), 1);
    assert_eq!(lattice.begin_nodes[0][0].borrow().id, 0);
    assert_eq!(lattice.begin_nodes[1][0].borrow().id, 0);
    assert_eq!(lattice.begin_nodes[2][0].borrow().id, 0);
}

#[test]
fn test_populate_nodes() {
    let pieces = vec![
        ("<unk>".to_string(), 0.0),
        ("a".to_string(), 0.1),
        ("b".to_string(), 0.2),
        ("ab".to_string(), 0.3),
        ("bc".to_string(), 0.4),
    ];
    let model = Unigram::from(pieces, Some(0), false).unwrap();

    let mut lattice = Lattice::from("abc", model.bos_id, model.eos_id);
    model.populate_nodes(&mut lattice);

    assert_eq!(lattice.begin_nodes[0].len(), 2); // a, ab
    assert_eq!(lattice.begin_nodes[1].len(), 2); // b, bc
    assert_eq!(lattice.begin_nodes[2].len(), 1); // c(unk)

    // Id is the vocabulary id from Unigram model
    assert_eq!(lattice.begin_nodes[0][0].borrow().id, 1);
    assert_eq!(lattice.begin_nodes[0][1].borrow().id, 3);
    assert_eq!(lattice.begin_nodes[1][0].borrow().id, 2);
    assert_eq!(lattice.begin_nodes[1][1].borrow().id, 4);
    assert_eq!(lattice.begin_nodes[2][0].borrow().id, 0);
}

#[test]
fn test_encode() {
    let sentencepieces = vec![
        ("<unk>".to_string(), 0.0),
        ("a".to_string(), 0.0),
        ("b".to_string(), 0.0),
        ("c".to_string(), 0.0),
        ("d".to_string(), 0.0),
        ("cd".to_string(), 1.0),
        ("ab".to_string(), 2.0),
        ("abc".to_string(), 5.0),
        ("abcd".to_string(), 10.0),
    ];

    let model = Unigram::from(sentencepieces, Some(0), false).unwrap();
    let result = model.encode("abcd").unwrap();
    assert_eq!(result, vec!["abcd"]);
}

#[test]
fn test_encode2() {
    let sentencepieces = vec![
        ("<unk>".to_string(), 0.0),
        ("ab".to_string(), 0.0),
        ("cd".to_string(), -0.1),
        ("abc".to_string(), -0.2),
        ("a".to_string(), -0.3),
        ("b".to_string(), -0.4),
        ("c".to_string(), -0.5),
        ("ABC".to_string(), -0.5),
        ("abcdabcd".to_string(), 20.0), // User defined just max the scores.
        ("q".to_string(), 20.5),
        ("r".to_string(), 20.5),
        ("qr".to_string(), -0.5),
    ];

    let mut model = Unigram::from(sentencepieces, Some(0), false).unwrap();

    for is_optimized in &[true, false] {
        model.set_optimized(*is_optimized);
        println!("IsOptimized {is_optimized:?}");
        assert_eq!(model.encode("abc").unwrap(), vec!["abc"]);
        assert_eq!(model.encode("AB").unwrap(), vec!["AB"]);

        model.set_fuse_unk(false);
        assert_eq!(model.encode("AB").unwrap(), vec!["A", "B"]);
        model.set_fuse_unk(true);
        assert_eq!(model.encode("AB").unwrap(), vec!["AB"]);

        assert_eq!(model.encode("abcd").unwrap(), vec!["ab", "cd"]);
        assert_eq!(model.encode("abcc").unwrap(), vec!["abc", "c"]);
        assert_eq!(
            model.encode("xabcabaabcdd").unwrap(),
            vec!["x", "abc", "ab", "a", "ab", "cd", "d"]
        );
        model.set_fuse_unk(false);
        assert_eq!(
            model.encode("xyz東京").unwrap(),
            vec!["x", "y", "z", "東", "京"]
        );
        model.set_fuse_unk(true);
        assert_eq!(model.encode("xyz東京").unwrap(), vec!["xyz東京"]);

        // User encoded in original version
        assert_eq!(model.encode("ABC").unwrap(), vec!["ABC"]);
        assert_eq!(model.encode("abABCcd").unwrap(), vec!["ab", "ABC", "cd"]);
        assert_eq!(
            model.encode("ababcdabcdcd").unwrap(),
            vec!["ab", "abcdabcd", "cd"]
        );
        assert_eq!(model.encode("abqrcd").unwrap(), vec!["ab", "q", "r", "cd"]);
    }
}

#[test]
fn test_unigram_bytefallback() {
    // In [97]: processor.encode_as_pieces("⅐⅛⅑ ")
    // Out[97]: ['▁', '<0xE2>', '<0x85>', '<0x90>', '⅛', '<0xE2>', '<0x85>', '<0x91>', '▁']
    let sentencepieces = vec![
        ("<unk>".to_string(), 0.0),
        ("<0xC3>".to_string(), -0.01),
        ("<0xA9>".to_string(), -0.03),
    ];
    let unigram = Unigram::from(sentencepieces, Some(0), true).unwrap();
    let tokens: Vec<Token> = unigram.tokenize("é").unwrap();
    assert_eq!(
        tokens,
        [
            Token {
                id: 1,
                value: "<0xC3>".to_string(),
                offsets: (0, 2)
            },
            Token {
                id: 2,
                value: "<0xA9>".to_string(),
                offsets: (0, 2)
            }
        ]
    );

    let tokens = unigram.tokenize("?é").unwrap();
    assert_eq!(tokens[0].id, 0);
}
