use super::{
    onnx::proto::tensor_proto::DataType,
    tensor::{Tensor, array},
};
use crate::binary16::{from_f32, to_f32};
use std::sync::Arc;

impl Tensor {
    pub(super) fn cast(&self, dest: DataType) -> Result<Self, String> {
        if self.dtype() == dest {
            return Ok(self.clone());
        }
        macro_rules! map_numeric {
            ($source:expr, $value:ident, $body:expr) => {{
                let values = $source.iter().map(|&$value| $body).collect();
                array(self.shape(), values)?
            }};
        }
        macro_rules! cast {
            ($t:ty, $variant:ident) => {
                Self::$variant(match self {
                    Self::Bool(a) => map_numeric!(a, v, u8::from(v) as $t),
                    Self::U8(a) => map_numeric!(a, v, v as $t),
                    Self::U16(a) => map_numeric!(a, v, v as $t),
                    Self::U32(a) => map_numeric!(a, v, v as $t),
                    Self::U64(a) => map_numeric!(a, v, v as $t),
                    Self::I8(a) => map_numeric!(a, v, v as $t),
                    Self::I16(a) => map_numeric!(a, v, v as $t),
                    Self::I32(a) => map_numeric!(a, v, v as $t),
                    Self::I64(a) => map_numeric!(a, v, v as $t),
                    Self::F16(a) => map_numeric!(a, v, to_f32(v) as $t),
                    Self::F32(a) => map_numeric!(a, v, v as $t),
                    Self::F64(a) => map_numeric!(a, v, v as $t),
                    Self::String(a) => array(
                        self.shape(),
                        a.iter()
                            .map(|v| {
                                std::str::from_utf8(v)
                                    .map_err(|e| e.to_string())?
                                    .parse::<$t>()
                                    .map_err(|e| e.to_string())
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    )?,
                })
            };
        }
        Ok(match dest {
            DataType::Uint8 => cast!(u8, U8),
            DataType::Uint16 => cast!(u16, U16),
            DataType::Uint32 => cast!(u32, U32),
            DataType::Uint64 => cast!(u64, U64),
            DataType::Int8 => cast!(i8, I8),
            DataType::Int16 => cast!(i16, I16),
            DataType::Int32 => cast!(i32, I32),
            DataType::Int64 => cast!(i64, I64),
            DataType::Float => cast!(f32, F32),
            DataType::Double => cast!(f64, F64),
            DataType::Float16 => match self {
                Self::F64(a) => Self::F16(a.mapv(crate::binary16::from_f64).into_shared()),
                _ => {
                    let Self::F32(a) = self.cast(DataType::Float)? else {
                        unreachable!()
                    };
                    Self::F16(a.mapv(from_f32).into_shared())
                }
            },
            DataType::Bool => {
                let Self::F64(a) = self.cast(DataType::Double)? else {
                    unreachable!()
                };
                Self::Bool(a.mapv(|v| v != 0.0).into_shared())
            }
            DataType::String => {
                macro_rules! strings {
                    ($a:expr,$v:ident,$val:expr) => {
                        array(
                            self.shape(),
                            $a.iter()
                                .map(|&$v| Arc::<[u8]>::from($val.to_string().into_bytes()))
                                .collect(),
                        )?
                    };
                }
                Self::String(match self {
                    Self::Bool(a) => strings!(a, v, u8::from(v)),
                    Self::U8(a) => strings!(a, v, v),
                    Self::U16(a) => strings!(a, v, v),
                    Self::U32(a) => strings!(a, v, v),
                    Self::U64(a) => strings!(a, v, v),
                    Self::I8(a) => strings!(a, v, v),
                    Self::I16(a) => strings!(a, v, v),
                    Self::I32(a) => strings!(a, v, v),
                    Self::I64(a) => strings!(a, v, v),
                    Self::F16(a) => strings!(a, v, to_f32(v)),
                    Self::F32(a) => strings!(a, v, v),
                    Self::F64(a) => strings!(a, v, v),
                    Self::String(a) => a.clone(),
                })
            }
            _ => return Err(format!("Unsupported ONNX cast type {dest:?}")),
        })
    }
}
