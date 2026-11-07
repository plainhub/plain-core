use crate::tokenizer::tokenizer::{Encoding, Result};
use serde::{Deserialize, Serialize};
use std::cmp;
use std::mem;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, Default)]
pub enum TruncationDirection {
    Left,
    #[default]
    Right,
}

impl std::convert::AsRef<str> for TruncationDirection {
    fn as_ref(&self) -> &str {
        match self {
            TruncationDirection::Left => "left",
            TruncationDirection::Right => "right",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncationParams {
    #[serde(default)]
    pub direction: TruncationDirection,
    pub max_length: usize,
    pub strategy: TruncationStrategy,
    pub stride: usize,
}

impl Default for TruncationParams {
    fn default() -> Self {
        Self {
            max_length: 512,
            strategy: TruncationStrategy::default(),
            stride: 0,
            direction: TruncationDirection::default(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TruncationError {
    #[error("Truncation error: Second sequence not provided")]
    SecondSequenceNotProvided,

    #[error("Truncation error: Sequence to truncate too short to respect the provided max_length")]
    SequenceTooShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, Default)]
pub enum TruncationStrategy {
    #[default]
    LongestFirst,
    OnlyFirst,
    OnlySecond,
}

impl std::convert::AsRef<str> for TruncationStrategy {
    fn as_ref(&self) -> &str {
        match self {
            Self::LongestFirst => "longest_first",
            Self::OnlyFirst => "only_first",
            Self::OnlySecond => "only_second",
        }
    }
}

pub fn truncate_encodings(
    mut encoding: Encoding,
    mut pair_encoding: Option<Encoding>,
    params: &TruncationParams,
) -> Result<(Encoding, Option<Encoding>)> {
    if params.max_length == 0 {
        encoding.truncate(0, params.stride, params.direction);
        if let Some(other_encoding) = pair_encoding.as_mut() {
            other_encoding.truncate(0, params.stride, params.direction);
        }
        return Ok((encoding, pair_encoding));
    }

    let total_length = encoding.get_ids().len()
        + pair_encoding
            .as_ref()
            .map(|e| e.get_ids().len())
            .unwrap_or(0);
    let to_remove = if total_length > params.max_length {
        total_length - params.max_length
    } else {
        return Ok((encoding, pair_encoding));
    };

    match params.strategy {
        TruncationStrategy::LongestFirst => {
            if let Some(other_encoding) = pair_encoding.as_mut() {
                // Assuming n1 <= n2, there are 3 cases
                // Case 1:
                //   No truncation needs to be performed.
                //   This scenario is handled before the match.
                // Case 2:
                //   Only the longer input needs to be truncated.
                //   n1 = n1
                //   n2 = max_length - n1
                // Case 3:
                //   Both inputs must be truncated.
                //   n1 = max_length / 2
                //   n2 = n1 + max_length % 2

                let mut n1 = encoding.get_ids().len();
                let mut n2 = other_encoding.get_ids().len();
                let mut swap = false;

                // Ensure n1 is the length of the shortest input
                if n1 > n2 {
                    swap = true;
                    mem::swap(&mut n1, &mut n2);
                }

                if n1 > params.max_length {
                    // This needs to be a special case
                    // to avoid max_length - n1 < 0
                    // since n1 and n2 are unsigned
                    n2 = n1;
                } else {
                    n2 = cmp::max(n1, params.max_length - n1);
                }

                if n1 + n2 > params.max_length {
                    n1 = params.max_length / 2;
                    n2 = n1 + params.max_length % 2;
                }

                // Swap lengths if we swapped previously
                if swap {
                    mem::swap(&mut n1, &mut n2);
                }
                encoding.truncate(n1, params.stride, params.direction);
                other_encoding.truncate(n2, params.stride, params.direction);
            } else {
                encoding.truncate(total_length - to_remove, params.stride, params.direction);
            }
        }
        TruncationStrategy::OnlyFirst | TruncationStrategy::OnlySecond => {
            let target = if params.strategy == TruncationStrategy::OnlyFirst {
                Ok(&mut encoding)
            } else if let Some(encoding) = pair_encoding.as_mut() {
                Ok(encoding)
            } else {
                Err(Box::new(TruncationError::SecondSequenceNotProvided))
            }?;

            let target_len = target.get_ids().len();
            if target_len > to_remove {
                target.truncate(target_len - to_remove, params.stride, params.direction);
            } else {
                return Err(Box::new(TruncationError::SequenceTooShort));
            }
        }
    }
    Ok((encoding, pair_encoding))
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/utils/truncation/tests.rs"]
mod tests;
