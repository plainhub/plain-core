use crate::tokenizer::tokenizer::Decoder;
use crate::tokenizer::tokenizer::pattern::Pattern;
use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};
use crate::tokenizer::utils::SysRegex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum ReplacePattern {
    String(String),
    Regex(String),
}

impl From<String> for ReplacePattern {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for ReplacePattern {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
struct ReplaceDeserializer {
    pattern: ReplacePattern,
    content: String,
}

impl std::convert::TryFrom<ReplaceDeserializer> for Replace {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn try_from(v: ReplaceDeserializer) -> Result<Self> {
        Self::new(v.pattern, v.content)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", try_from = "ReplaceDeserializer")]
pub struct Replace {
    pattern: ReplacePattern,
    pub content: String,
    #[serde(skip)]
    regex: SysRegex,
}

impl Clone for Replace {
    fn clone(&self) -> Self {
        Self::new(self.pattern.clone(), &self.content).unwrap()
    }
}

impl PartialEq for Replace {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.content == other.content
    }
}

impl Replace {
    pub fn new<I: Into<ReplacePattern>, C: Into<String>>(pattern: I, content: C) -> Result<Self> {
        let pattern: ReplacePattern = pattern.into();
        let regex = match &pattern {
            ReplacePattern::String(s) => SysRegex::new(&crate::regex::escape(s))?,
            ReplacePattern::Regex(r) => SysRegex::new(r)?,
        };

        Ok(Self {
            pattern,
            content: content.into(),
            regex,
        })
    }
}

impl Normalizer for Replace {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        normalized.replace(&self.regex, &self.content)
    }
}

impl Decoder for Replace {
    fn decode_chain(&self, tokens: Vec<String>) -> Result<Vec<String>> {
        tokens
            .into_iter()
            .map(|token| -> Result<String> {
                let mut new_token = "".to_string();

                for ((start, stop), is_match) in (&self.regex).find_matches(&token)? {
                    if is_match {
                        new_token.push_str(&self.content);
                    } else {
                        new_token.push_str(&token[start..stop]);
                    }
                }
                Ok(new_token)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/replace/tests.rs"]
mod tests;
