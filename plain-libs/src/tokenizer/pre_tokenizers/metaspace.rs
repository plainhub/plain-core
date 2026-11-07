use crate::tokenizer::tokenizer::{
    Decoder, PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior,
};
use serde::{Deserialize, Deserializer, Serialize, de};

#[derive(Debug, Clone, PartialEq, Serialize, Eq, Deserialize, Copy)]
#[serde(rename_all = "snake_case")]
pub enum PrependScheme {
    First,

    Never,

    Always,
}

impl std::fmt::Display for PrependScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.serialize(f)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Eq)]
#[serde(tag = "type")]
pub struct Metaspace {
    replacement: char,
    pub prepend_scheme: PrependScheme,
    pub split: bool,
    #[serde(skip)]
    str_rep: String,
}

impl<'de> Deserialize<'de> for Metaspace {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Type {
            Metaspace,
        }

        fn default_prepend_scheme_value() -> PrependScheme {
            PrependScheme::Always
        }

        #[derive(Deserialize)]
        pub struct MetaspaceHelper {
            #[serde(rename = "type")]
            _type: Type,
            replacement: char,

            pub add_prefix_space: Option<bool>,
            #[serde(default = "default_prepend_scheme_value")]
            pub prepend_scheme: PrependScheme,
            pub split: Option<bool>,
            #[serde(rename = "str_rep")]
            _str_rep: Option<String>,
        }

        let mut helper = MetaspaceHelper::deserialize(deserializer)?;
        if let Some(false) = helper.add_prefix_space {
            if helper.prepend_scheme != PrependScheme::Never {
                return Err(de::Error::custom(
                    "add_prefix_space does not match declared prepend_scheme",
                ));
            }
            helper.prepend_scheme = PrependScheme::Never;
        }
        let instance = Self::new(
            helper.replacement,
            helper.prepend_scheme,
            helper.split.unwrap_or(true),
        );
        Ok(instance)
    }
}

impl Metaspace {
    pub fn new(replacement: char, prepend_scheme: PrependScheme, split: bool) -> Self {
        Self {
            replacement,
            str_rep: replacement.to_string(),
            prepend_scheme,
            split,
        }
    }

    pub fn get_replacement(&self) -> char {
        self.replacement
    }

    pub fn set_replacement(&mut self, replacement: char) {
        self.replacement = replacement;
        self.str_rep = replacement.to_string();
    }

    pub fn get_split(&self) -> bool {
        self.split
    }

    pub fn set_split(&mut self, split: bool) {
        self.split = split;
    }

    pub fn get_prepend_scheme(&self) -> PrependScheme {
        self.prepend_scheme
    }

    pub fn set_prepend_scheme(&mut self, scheme: PrependScheme) {
        self.prepend_scheme = scheme;
    }
}

impl Default for Metaspace {
    fn default() -> Self {
        Self::new('▁', PrependScheme::Always, true)
    }
}

impl PreTokenizer for Metaspace {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, mut normalized| {
            normalized.replace(' ', &self.str_rep)?;
            match self.prepend_scheme {
                PrependScheme::Always => {
                    if !normalized.get().starts_with(self.replacement) {
                        normalized.prepend(&self.str_rep);
                    }
                }
                PrependScheme::First => {
                    if !normalized.get().starts_with(self.replacement)
                        && normalized.offsets_original().0 == 0
                    {
                        normalized.prepend(&self.str_rep);
                    }
                }
                PrependScheme::Never => {}
            };
            if self.split {
                normalized.split(self.replacement, SplitDelimiterBehavior::MergedWithNext)
            } else {
                Ok(vec![normalized])
            }
        })
    }
}

impl Decoder for Metaspace {
    fn decode_chain(&self, tokens: Vec<String>) -> Result<Vec<String>> {
        Ok(tokens
            .iter()
            .enumerate()
            .map(|(i, token)| {
                token
                    .chars()
                    .flat_map(|c| {
                        if c == self.replacement {
                            if i == 0 && self.prepend_scheme != PrependScheme::Never {
                                None
                            } else {
                                Some(' ')
                            }
                        } else {
                            Some(c)
                        }
                    })
                    .collect::<String>()
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/metaspace/tests.rs"]
mod tests;
