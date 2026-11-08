use crate::tokenizer::pre_tokenizers::unicode_scripts::scripts::{Script, get_script};
use crate::tokenizer::tokenizer::{PreTokenizedString, PreTokenizer, Result, normalizer::Range};

impl_serde_type! {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct UnicodeScripts;
}

impl UnicodeScripts {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for UnicodeScripts {
    fn default() -> Self {
        Self::new()
    }
}

// This code exists in the Unigram default IsValidSentencePiece.
// It could be integrated directly within `get_script` but I
// think it's kind of tricky to see those modifications later
// I am guessing release mode will optimize this away anyway.
fn fixed_script(c: char) -> Script {
    let raw_script = get_script(c);
    if c as u32 == 0x30FC {
        Script::Han
    } else if c == ' ' {
        Script::Any
    } else {
        match raw_script {
            Script::Hiragana => Script::Han,
            Script::Katakana => Script::Han,
            script => script,
        }
    }
}

impl PreTokenizer for UnicodeScripts {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, normalized| {
            let mut last_script = None;
            let mut offset = 0;
            let mut ranges: Vec<_> = normalized
                .get()
                .chars()
                .filter_map(|c| {
                    let script = Some(fixed_script(c));
                    let result = if script != Some(Script::Any)
                        && last_script != Some(Script::Any)
                        && last_script != script
                    {
                        Some(offset)
                    } else {
                        None
                    };
                    offset += c.len_utf8();
                    if script != Some(Script::Any) {
                        last_script = script;
                    }

                    result
                })
                .collect();
            ranges.push(normalized.get().len());
            Ok(ranges
                .windows(2)
                .map(|item| {
                    normalized
                        .slice(Range::Normalized(item[0]..item[1]))
                        .expect("NormalizedString bad split")
                })
                .collect::<Vec<_>>())
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/unicode_scripts/pre_tokenizer/tests.rs"]
mod tests;
