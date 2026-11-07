use crate::tokenizer::tokenizer::{Encoding, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PaddingDirection {
    Left,
    Right,
}

impl std::convert::AsRef<str> for PaddingDirection {
    fn as_ref(&self) -> &str {
        match self {
            PaddingDirection::Left => "left",
            PaddingDirection::Right => "right",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaddingParams {
    pub strategy: PaddingStrategy,
    pub direction: PaddingDirection,
    pub pad_to_multiple_of: Option<usize>,
    pub pad_id: u32,
    pub pad_type_id: u32,
    pub pad_token: String,
}

impl Default for PaddingParams {
    fn default() -> Self {
        Self {
            strategy: PaddingStrategy::BatchLongest,
            direction: PaddingDirection::Right,
            pad_to_multiple_of: None,
            pad_id: 0,
            pad_type_id: 0,
            pad_token: String::from("[PAD]"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaddingStrategy {
    BatchLongest,
    Fixed(usize),
}

pub fn pad_encodings(encodings: &mut [Encoding], params: &PaddingParams) -> Result<()> {
    if encodings.is_empty() {
        return Ok(());
    }

    let mut pad_length = match params.strategy {
        PaddingStrategy::Fixed(size) => size,
        PaddingStrategy::BatchLongest => encodings.iter().map(|e| e.get_ids().len()).max().unwrap(),
    };

    if let Some(multiple) = params.pad_to_multiple_of {
        if multiple > 0 && pad_length % multiple > 0 {
            pad_length += multiple - pad_length % multiple;
        }
    }

    encodings.iter_mut().for_each(|encoding| {
        encoding.pad(
            pad_length,
            params.pad_id,
            params.pad_type_id,
            &params.pad_token,
            params.direction,
        )
    });

    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/utils/padding/tests.rs"]
mod tests;
