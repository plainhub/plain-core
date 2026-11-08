use super::*;
use crate::tokenizer::normalizers::NormalizerWrapper;
use crate::tokenizer::normalizers::byte_level::ByteLevel as ByteLevelNormalizer;
use crate::tokenizer::normalizers::utils::Lowercase;
use crate::tokenizer::{OffsetReferential, OffsetType, Result, Token};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
struct ModelMock {
    vocab: HashMap<String, u32>,
    vocab_r: HashMap<u32, String>,
}
impl ModelMock {
    pub fn new<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = &'static (&'static str, u32)>,
    {
        let vocab: HashMap<String, u32> = iter
            .into_iter()
            .map(|&(tok, id)| (tok.to_string(), id))
            .collect();
        Self {
            vocab_r: vocab
                .iter()
                .map(|(tok, id)| (*id, tok.to_owned()))
                .collect(),
            vocab,
        }
    }
}

fn simplify_output(result: &'_ PreTokenizedString) -> Vec<(&'_ str, Option<Vec<u32>>)> {
    result
        .get_splits(OffsetReferential::Original, OffsetType::Byte)
        .into_iter()
        .map(|(s, _, tokens)| {
            (
                s,
                tokens
                    .as_ref()
                    .map(|t| t.iter().map(|t| t.id).collect::<Vec<_>>()),
            )
        })
        .collect::<Vec<_>>()
}

impl Model for ModelMock {
    fn tokenize(&self, _sequence: &str) -> Result<Vec<Token>> {
        unimplemented!()
    }
    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.vocab.get(token).copied()
    }
    fn id_to_token(&self, id: u32) -> Option<String> {
        self.vocab_r.get(&id).cloned()
    }
    fn get_vocab(&self) -> HashMap<String, u32> {
        self.vocab.clone().into_iter().collect()
    }
    fn get_vocab_size(&self) -> usize {
        self.vocab.len()
    }
}

#[test]
fn can_add_tokens() {
    let model = ModelMock::new(&[("test", 0), ("tost", 1)]);
    let mut vocab = AddedVocabulary::new();
    let normalizer: Option<&NormalizerWrapper> = None;

    // Add tokens normally
    assert_eq!(
        vocab.add_tokens(
            &[AddedToken::from("added_token_1", false)],
            &model,
            normalizer
        ),
        1
    );

    let vocab_len: usize = vocab.len();
    assert_eq!(vocab_len, 1);

    // Does not add multiple time the same token
    assert_eq!(
        vocab.add_tokens(
            &[
                AddedToken::from("added_token_2", false),
                AddedToken::from("added_token_2", false)
            ],
            &model,
            normalizer
        ),
        1
    );
    assert_eq!(vocab.len(), 2);

    // Also adds tokens already covered by the model
    let added_token = AddedToken::from("test", false);
    assert_eq!(
        vocab.add_tokens(std::slice::from_ref(&added_token), &model, normalizer),
        1
    );
    assert_eq!(vocab.len(), 3);

    assert_eq!(vocab.get_added_tokens_decoder()[&0], added_token);
}

#[test]
fn can_add_special_tokens() {
    let model = ModelMock::new(&[("test", 0), ("tost", 1)]);
    let mut vocab = AddedVocabulary::new();
    let normalizer: Option<&NormalizerWrapper> = None;
    // Add tokens normally
    assert_eq!(
        vocab.add_special_tokens(
            &[AddedToken::from("added_token_1", true)],
            &model,
            normalizer
        ),
        1
    );
    assert_eq!(vocab.len(), 1);

    // Does not add multiple time the same token
    assert_eq!(
        vocab.add_special_tokens(
            &[
                AddedToken::from("added_token_2", true),
                AddedToken::from("added_token_2", true)
            ],
            &model,
            normalizer
        ),
        1
    );
    assert_eq!(vocab.len(), 2);

    // Can add tokens already covered by the model
    assert_eq!(
        vocab.add_special_tokens(&[AddedToken::from("test", true)], &model, normalizer),
        1
    );
    assert_eq!(vocab.len(), 3); // New token was added
    assert!(vocab.is_special_token("test"));
    assert_eq!(
        *vocab.get_added_tokens_decoder(),
        HashMap::from([
            (0, AddedToken::from("test", true)),
            (2, AddedToken::from("added_token_1", true)),
            (3, AddedToken::from("added_token_2", true)),
        ])
    );
    assert!(vocab.added_tokens_map.contains_key("test"));
    assert!(vocab.added_tokens_map_r.contains_key(&0));

    vocab.add_tokens(
        &[
            AddedToken::from("tost", true),
            AddedToken::from("another_two", false),
        ],
        &model,
        normalizer,
    );
    assert_eq!(vocab.len(), 5); // New token was added
    assert_eq!(vocab.get_vocab()["another_two"], 4); // New token was added, but the index is not the length of the vocab

    // Let's add an already added token again
    assert_eq!(
        vocab.add_special_tokens(&[AddedToken::from("another_two", true)], &model, normalizer),
        1
    );
    assert_eq!(vocab.len(), 5); // Token was already there
    assert_eq!(vocab.get_vocab()["another_two"], 4); // Token idx not changed

    // Just checking that we can set the content of the string in rust
    let mut token: AddedToken = AddedToken::from("Hey", false);
    token.content = "hey".to_string();
    assert_eq!(token.content, "hey"); // Token was already there

    token.special = true;
    assert!(token.special); // Token was already there
}

#[test]
fn can_extract_added_tokens() {
    // Is able to extract both normal and special tokens
    let model = ModelMock::new(&[]);
    let mut vocab = AddedVocabulary::new();
    let normalizer: Option<&NormalizerWrapper> = None;

    vocab.add_tokens(
        &[
            AddedToken::from("my", false),
            AddedToken::from("name", false),
        ],
        &model,
        normalizer,
    );
    vocab.add_special_tokens(
        &[
            AddedToken::from("[CLS]", true),
            AddedToken::from("[SEP]", true),
        ],
        &model,
        normalizer,
    );

    let result = vocab.extract_and_normalize(normalizer, "[CLS] My name is Anthony [SEP]");
    assert_eq!(
        result
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, _, tokens)| (
                s,
                tokens
                    .as_ref()
                    .map(|t| t.iter().map(|t| t.id).collect::<Vec<_>>())
            ))
            .collect::<Vec<_>>(),
        vec![
            ("[CLS]", Some(vec![2])),
            (" My ", None),
            ("name", Some(vec![1])),
            (" is Anthony ", None),
            ("[SEP]", Some(vec![3]))
        ]
    );
}

#[test]
fn options_use_cases() {
    // Is able to extract both normal and special tokens, with various options (lstrip, rstrip,
    // single_word, normalized)
    let model = ModelMock::new(&[]);
    let normalizer = Lowercase;
    let mut vocab = AddedVocabulary::new();

    vocab.add_tokens(
        &[
            AddedToken::from("my", false).lstrip(true).rstrip(true),
            AddedToken::from("name", false),
            AddedToken::from("ony", false).single_word(true),
        ],
        &model,
        Some(&normalizer),
    );
    vocab.add_special_tokens(
        &[
            AddedToken::from("[CLS]", true),
            AddedToken::from("[SEP]", true),
        ],
        &model,
        Some(&normalizer),
    );

    let result = vocab.extract_and_normalize(Some(&normalizer), "[CLS] My name is Anthony [SEP]");

    assert_eq!(
        simplify_output(&result),
        vec![
            ("[CLS]", Some(vec![3])),
            // This one includes both spaces because of the lstrip & rstrip
            // And it matches because normalized == true
            (" my ", Some(vec![0])),
            ("name", Some(vec![1])),
            // `ony` is not extracted here thanks to single_word
            (" is anthony ", None),
            ("[SEP]", Some(vec![4])),
        ]
    );
}

#[test]
fn empty_matches() {
    let vocab = AddedVocabulary::new();
    let matches = vocab.find_matches("", &vocab.split_trie);
    assert_eq!(matches, vec![(None, (0, 0))]);
}

#[test]
fn test_single_word_is_correct() {
    // Is able to extract both normal and special tokens, with various options (lstrip, rstrip,
    // single_word, normalized)
    let model = ModelMock::new(&[]);
    let mut vocab = AddedVocabulary::new();
    let normalizer = Lowercase;

    vocab.add_tokens(
        &[AddedToken::from("<mask>", false).single_word(true)],
        &model,
        Some(&normalizer),
    );
    // Left, in the middle, non single world left, non single word right, end of sentence valid
    let result = vocab.extract_and_normalize(
        Some(&normalizer),
        "<mask> My name <mask> A<mask> <mask>ony <mask>",
    );
    assert_eq!(
        simplify_output(&result),
        vec![
            ("<mask>", Some(vec![0])),
            (" my name ", None),
            ("<mask>", Some(vec![0])),
            (" a<mask> <mask>ony ", None),
            ("<mask>", Some(vec![0]))
        ]
    );
}

#[test]
fn test_single_word_is_unicode_correct() {
    let model = ModelMock::new(&[]);
    let mut vocab = AddedVocabulary::new();
    let normalizer = Lowercase;

    assert_eq!(vocab.len(), 0);

    vocab.add_tokens(
        &[AddedToken::from("<mask>", false).single_word(true)],
        &model,
        Some(&normalizer),
    );
    let result = vocab.extract_and_normalize(Some(&normalizer), "<mask>, <mask>- ◌̰<mask>");
    assert_eq!(
        simplify_output(&result),
        vec![
            // Punctuation is not word
            ("<mask>", Some(vec![0])),
            (", ", None),
            // dash is not word
            ("<mask>", Some(vec![0])),
            // This is unicode combining mark character and is word: https://en.wikipedia.org/wiki/Combining_Diacritical_Marks
            ("- ◌̰<mask>", None),
        ]
    );
}

#[test]
fn test_lstrip_unicode_space() {
    let model = ModelMock::new(&[]);
    let mut vocab = AddedVocabulary::new();
    let normalizer = Lowercase;

    vocab.add_tokens(
        &[AddedToken::from("<mask>", false)
            .lstrip(true)
            .rstrip(true)
            .single_word(true)],
        &model,
        Some(&normalizer),
    );
    let result =
        vocab.extract_and_normalize(Some(&normalizer), "Hi <mask> there\t<mask>\t<mask>\u{2000}");
    assert_eq!(
        simplify_output(&result),
        vec![
            ("hi", None),
            // Regular space
            (" <mask> ", Some(vec![0])),
            ("there", None),
            // \t is a spacing character
            ("\t<mask>\t", Some(vec![0])),
            // Non overlapping
            // \u{2000} is mongolian vowel separator: https://jkorpela.fi/chars/spaces.html
            ("<mask>\u{2000}", Some(vec![0])),
        ]
    );
}

#[test]
fn test_encode_special_tokens() {
    let model = ModelMock::new(&[]);
    let mut vocab = AddedVocabulary::new();
    let normalizer = Lowercase;

    vocab.add_tokens(
        &[
            AddedToken::from("<mask>", true)
                .lstrip(true)
                .rstrip(true)
                .single_word(true),
            AddedToken::from("ask>", false),
            AddedToken::from("<pad>", true),
        ],
        &model,
        Some(&normalizer),
    );
    vocab.set_encode_special_tokens(true);

    let result = vocab.extract_and_normalize(
        Some(&normalizer),
        "Hi <mask> there\t<mask>\t<mask>\u{2000} <pad> <mask><pad><pad>",
    );

    assert_eq!(
        simplify_output(&result),
        vec![
            ("hi <m", None),
            ("ask>", Some(vec![1])),
            (" there\t<m", None),
            ("ask>", Some(vec![1])),
            ("\t<m", None),
            ("ask>", Some(vec![1])),
            ("\u{2000} <pad> <m", None),
            ("ask>", Some(vec![1])),
            ("<pad><pad>", None)
        ]
    );

    vocab.set_encode_special_tokens(false);

    let result = vocab.extract_and_normalize(
        Some(&normalizer),
        "Hi <mask> there\t<mask>\t<mask>\u{2000} <pad> <mask><pad><pad>",
    );
    assert_eq!(
        simplify_output(&result),
        vec![
            ("hi", None),
            (" <mask> ", Some(vec![0])),
            ("there", None),
            ("\t<mask>\t", Some(vec![0])),
            ("<mask>\u{2000} ", Some(vec![0])),
            ("<pad>", Some(vec![2])),
            (" <mask>", Some(vec![0])),
            ("<pad>", Some(vec![2])),
            ("<pad>", Some(vec![2]))
        ]
    );
}
#[test]
fn byte_level_normalizer() {
    // Is able to extract both normal and special tokens
    let model = ModelMock::new(&[]);
    let mut vocab = AddedVocabulary::new();
    let from = NormalizerWrapper::from(ByteLevelNormalizer::new());
    let normalizer: Option<&NormalizerWrapper> = Some(&from);

    vocab.add_tokens(
        &[AddedToken::from("my", false), AddedToken::from("今", false)],
        &model,
        normalizer,
    );
    let result = vocab.extract_and_normalize(normalizer, "my今");
    assert_eq!(
        result
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, _, tokens)| (
                s,
                tokens
                    .as_ref()
                    .map(|t| t.iter().map(|t| t.id).collect::<Vec<_>>())
            ))
            .collect::<Vec<_>>(),
        vec![("my", Some(vec![0])), ("ä»Ĭ", Some(vec![1])),]
    );
}
