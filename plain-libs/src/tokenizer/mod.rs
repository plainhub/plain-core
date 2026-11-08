#![forbid(unsafe_code)]

#[macro_use]
pub mod utils;
pub mod decoders;
mod graphemes;
mod normalization;
pub mod models;
pub mod normalizers;
pub mod pre_tokenizers;
pub mod processors;
pub mod tokenizer;
mod unicode_categories;

// Re-export from tokenizer
pub use tokenizer::*;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/tokenizer/inference.rs"]
mod inference_tests;
