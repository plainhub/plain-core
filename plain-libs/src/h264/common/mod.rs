pub fn knob(name: &str) -> Option<std::string::String> {
    std::env::var(name).ok()
}
macro_rules! cached_knob {
    ($ ty : ty , $ init : expr) => {{
        {
            static V: ::std::sync::OnceLock<$ty> = ::std::sync::OnceLock::new();
            *V.get_or_init(|| $init)
        }
    }};
    ($ ty : ty , $ default : expr , $ init : expr) => {{
        let _ = $default;
        $crate::h264::common::cached_knob!($ty, $init)
    }};
}
pub mod aligned;
pub mod arms;
pub mod bit_reader;
pub mod cabac_tables;
pub mod cavlc;
pub mod deblock;
pub const ACCEL: bool = cfg!(any());
pub mod inter;
pub mod nal;
pub mod predict;
pub mod once {
    pub use std::sync::OnceLock;
}

pub mod prof;
pub mod transform;
pub mod types;
pub use bit_reader::BitReader;
pub use nal::NalUnitType;
pub use types::{ChromaFormat, Profile, YuvFrame, YuvPlanes};

pub(crate) use cached_knob;
