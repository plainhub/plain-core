use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};

use serde::{Deserialize, Serialize};
use unicode_categories::UnicodeCategories;

fn is_whitespace(c: char) -> bool {
    // These are technically control characters but we count them as whitespace
    match c {
        '\t' | '\n' | '\r' => true,
        _ => c.is_whitespace(),
    }
}

fn is_control(c: char) -> bool {
    // These are technically control characters but we count them as whitespace
    match c {
        '\t' | '\n' | '\r' => false,
        // The definition of `is_control` here is quite large and contains also
        // Cc, Cf, Cn or Co
        // cf. https://unicode.org/reports/tr44/ (Table 12)
        _ => c.is_other(),
    }
}

fn is_chinese_char(c: char) -> bool {
    matches!(
        c as usize,
        0x4E00..=0x9FFF |
        0x3400..=0x4DBF |
        0x20000..=0x2A6DF |
        0x2A700..=0x2B73F |
        0x2B740..=0x2B81F |
        0x2B920..=0x2CEAF |
        0xF900..=0xFAFF |
        0x2F800..=0x2FA1F
    )
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub struct BertNormalizer {
    pub clean_text: bool,

    pub handle_chinese_chars: bool,

    pub strip_accents: Option<bool>,

    pub lowercase: bool,
}

impl Default for BertNormalizer {
    fn default() -> Self {
        Self {
            clean_text: true,
            handle_chinese_chars: true,
            strip_accents: None,
            lowercase: true,
        }
    }
}

impl BertNormalizer {
    pub fn new(
        clean_text: bool,
        handle_chinese_chars: bool,
        strip_accents: Option<bool>,
        lowercase: bool,
    ) -> Self {
        Self {
            clean_text,
            handle_chinese_chars,
            strip_accents,
            lowercase,
        }
    }

    fn do_clean_text(&self, normalized: &mut NormalizedString) {
        normalized
            .filter(|c| !(c as usize == 0 || c as usize == 0xfffd || is_control(c)))
            .map(|c| if is_whitespace(c) { ' ' } else { c });
    }

    fn do_handle_chinese_chars(&self, normalized: &mut NormalizedString) {
        let mut new_chars: Vec<(char, isize)> = vec![];
        normalized.for_each(|c| {
            if is_chinese_char(c) {
                new_chars.extend([(' ', 0), (c, 1), (' ', 1)]);
            } else {
                new_chars.push((c, 0));
            }
        });
        normalized.transform(new_chars, 0);
    }

    fn do_strip_accents(&self, normalized: &mut NormalizedString) {
        normalized.nfd().filter(|c| !c.is_mark_nonspacing());
    }

    fn do_lowercase(&self, normalized: &mut NormalizedString) {
        normalized.lowercase();
    }
}

impl Normalizer for BertNormalizer {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        if self.clean_text {
            self.do_clean_text(normalized);
        }
        if self.handle_chinese_chars {
            self.do_handle_chinese_chars(normalized);
        }
        let strip_accents = self.strip_accents.unwrap_or(self.lowercase);
        if strip_accents {
            self.do_strip_accents(normalized);
        }
        if self.lowercase {
            self.do_lowercase(normalized);
        }

        Ok(())
    }
}
