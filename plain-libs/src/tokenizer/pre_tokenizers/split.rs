use crate::tokenizer::utils::SysRegex;
use serde::{Deserialize, Deserializer, Serialize};

use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior, pattern::Invert,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum SplitPattern {
    String(String),
    Regex(String),
}

impl From<String> for SplitPattern {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for SplitPattern {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub struct Split {
    pub pattern: SplitPattern,
    #[serde(skip)]
    pub regex: SysRegex,
    pub behavior: SplitDelimiterBehavior,
    pub invert: bool,
}

impl<'de> Deserialize<'de> for Split {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Type {
            Split,
        }

        #[derive(Deserialize)]
        pub struct SplitHelper {
            #[serde(rename = "type")]
            _type: Type,
            pattern: SplitPattern,
            behavior: SplitDelimiterBehavior,
            invert: bool,
        }

        let helper = SplitHelper::deserialize(deserializer)?;
        Self::new(helper.pattern, helper.behavior, helper.invert).map_err(serde::de::Error::custom)
    }
}

impl Clone for Split {
    fn clone(&self) -> Self {
        Self::new(self.pattern.clone(), self.behavior, self.invert).unwrap()
    }
}

impl PartialEq for Split {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
            && self.behavior == other.behavior
            && self.invert == other.invert
    }
}

impl Split {
    pub fn new<I: Into<SplitPattern>>(
        pattern: I,
        behavior: SplitDelimiterBehavior,
        invert: bool,
    ) -> Result<Self> {
        let pattern: SplitPattern = pattern.into();
        let regex = match &pattern {
            SplitPattern::String(s) => SysRegex::new(&crate::regex::escape(s))?,
            SplitPattern::Regex(r) => SysRegex::new(r)?,
        };

        Ok(Self {
            pattern,
            regex,
            behavior,
            invert,
        })
    }
}

impl PreTokenizer for Split {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        if self.invert {
            pretokenized.split(|_, normalized| normalized.split(Invert(&self.regex), self.behavior))
        } else {
            pretokenized.split(|_, normalized| normalized.split(&self.regex, self.behavior))
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/split/tests.rs"]
mod tests;
