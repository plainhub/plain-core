pub mod __private {
    pub use alloc::vec;
}
pub use crate::serde_json::de::from_reader;
pub use crate::serde_json::de::{Deserializer, StreamDeserializer, from_slice, from_str};
pub use crate::serde_json::error::{Error, Result};
pub use crate::serde_json::ser::{Serializer, to_writer, to_writer_pretty};
pub use crate::serde_json::ser::{to_string, to_string_pretty, to_vec, to_vec_pretty};
pub use crate::serde_json::value::{Map, Number, Value, from_value, to_value};
macro_rules! tri {
    ($e : expr $(,) ?) => {
        match $e {
            core::result::Result::Ok(val) => val,
            core::result::Result::Err(err) => return core::result::Result::Err(err),
        }
    };
}
#[macro_use]
mod macros;
pub mod de;
pub mod error;
mod io;
mod iter;
pub mod map;
mod number;
mod raw;
mod read;
pub mod ser;
pub mod value;
pub use crate::{
    plain_serde_json_json as json,
    plain_serde_json_json_expect_expr_comma as json_expect_expr_comma,
    plain_serde_json_json_internal as json_internal,
    plain_serde_json_json_internal_vec as json_internal_vec,
    plain_serde_json_json_unexpected as json_unexpected,
};

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/serde_json.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/serde_json/codegen.rs"]
mod codegen_tests;
