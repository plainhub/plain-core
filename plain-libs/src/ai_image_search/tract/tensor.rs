use super::onnx::proto::{SparseTensorProto, TensorProto, tensor_proto::DataType};
use ndarray::{ArcArray, ArrayD, IxDyn};
use std::sync::Arc;

pub(super) type Array<T> = ArcArray<T, IxDyn>;

#[derive(Clone)]
pub(super) enum Tensor {
    Bool(Array<bool>),
    U8(Array<u8>),
    U16(Array<u16>),
    U32(Array<u32>),
    U64(Array<u64>),
    I8(Array<i8>),
    I16(Array<i16>),
    I32(Array<i32>),
    I64(Array<i64>),
    F16(Array<u16>),
    F32(Array<f32>),
    F64(Array<f64>),
    String(Array<Arc<[u8]>>),
}

pub(super) fn checked_volume(shape: &[usize]) -> Option<usize> {
    shape
        .iter()
        .try_fold(1usize, |size, &dim| size.checked_mul(dim))
}

pub(super) fn array<T: Clone>(shape: &[usize], data: Vec<T>) -> Result<Array<T>, String> {
    ArrayD::from_shape_vec(IxDyn(shape), data)
        .map(|array| array.into_shared())
        .map_err(|e| e.to_string())
}

impl Tensor {
    pub(super) fn from_proto(tensor: TensorProto) -> Result<Self, String> {
        let shape = tensor
            .dims
            .iter()
            .map(|&n| usize::try_from(n).map_err(|_| format!("Invalid ONNX tensor dimension {n}")))
            .collect::<Result<Vec<_>, _>>()?;
        let elements = checked_volume(&shape).ok_or("ONNX tensor shape exceeds address space")?;
        let dtype = DataType::try_from(tensor.data_type).map_err(|e| e.to_string())?;
        if tensor.segment.is_some() {
            return Err("Segmented ONNX tensor data is unsupported".into());
        }
        macro_rules! load {
            ($variant:ident, $ty:ty, $data:expr) => {{
                let data: Vec<$ty> = if !tensor.raw_data.is_empty() {
                    let size = std::mem::size_of::<$ty>();
                    if elements.checked_mul(size) != Some(tensor.raw_data.len()) {
                        return Err(format!(
                            "ONNX tensor {} payload does not match shape {shape:?}",
                            tensor.name
                        ));
                    }
                    tensor
                        .raw_data
                        .chunks_exact(size)
                        .map(|b| <$ty>::from_le_bytes(b.try_into().unwrap()))
                        .collect()
                } else {
                    $data
                };
                Ok(Self::$variant(array(&shape, data)?))
            }};
        }
        match dtype {
            DataType::Float => load!(F32, f32, tensor.float_data),
            DataType::Double => load!(F64, f64, tensor.double_data),
            DataType::Float16 => load!(
                F16,
                u16,
                tensor.int32_data.into_iter().map(|v| v as u16).collect()
            ),
            DataType::Int64 => load!(I64, i64, tensor.int64_data),
            DataType::Int32 => load!(I32, i32, tensor.int32_data),
            DataType::Int16 => load!(
                I16,
                i16,
                tensor.int32_data.into_iter().map(|v| v as i16).collect()
            ),
            DataType::Int8 => load!(
                I8,
                i8,
                tensor.int32_data.into_iter().map(|v| v as i8).collect()
            ),
            DataType::Uint64 => load!(U64, u64, tensor.uint64_data),
            DataType::Uint32 => load!(
                U32,
                u32,
                tensor.uint64_data.into_iter().map(|v| v as u32).collect()
            ),
            DataType::Uint16 => load!(
                U16,
                u16,
                tensor.int32_data.into_iter().map(|v| v as u16).collect()
            ),
            DataType::Uint8 => load!(
                U8,
                u8,
                tensor.int32_data.into_iter().map(|v| v as u8).collect()
            ),
            DataType::Bool => {
                let data = if tensor.raw_data.is_empty() {
                    tensor.int32_data.into_iter().map(|v| v != 0).collect()
                } else {
                    tensor.raw_data.iter().map(|&v| v != 0).collect()
                };
                Ok(Self::Bool(array(&shape, data)?))
            }
            DataType::String => Ok(Self::String(array(
                &shape,
                tensor.string_data.into_iter().map(Arc::from).collect(),
            )?)),
            _ => Err(format!("Unsupported ONNX tensor type {dtype:?}")),
        }
    }
}

macro_rules! dispatch {
    ($tensor:expr, |$value:ident, $kind:ident| $body:expr) => {
        dispatch!(@cases $tensor, $value, $kind, $body; Bool, U8, U16, U32, U64, I8, I16, I32, I64, F16, F32, F64, String)
    };
    (@cases $tensor:expr, $value:ident, $kind:ident, $body:expr; $($ty:ident),+) => {
        match $tensor {
            $(Tensor::$ty($value) => {
                use Tensor::$ty as $kind;
                $body
            }),+
        }
    };
}
pub(super) use dispatch;

impl Tensor {
    pub(super) fn from_sparse(sparse: SparseTensorProto) -> Result<Self, String> {
        let dims = sparse
            .dims
            .into_iter()
            .map(|n| usize::try_from(n).map_err(|_| "Invalid sparse tensor shape"))
            .collect::<Result<Vec<_>, _>>()?;
        let values = Self::from_proto(sparse.values.ok_or("Missing sparse tensor values")?)?;
        let &[count] = values.shape() else {
            return Err("Expected rank-one sparse tensor values".into());
        };
        let Self::I64(indices) =
            Self::from_proto(sparse.indices.ok_or("Missing sparse tensor indices")?)?
        else {
            return Err("Expected int64 sparse tensor indices".into());
        };
        let mut offsets = Vec::with_capacity(count);
        match indices.shape() {
            [n] if *n == count => {
                for &i in &indices {
                    offsets.push(usize::try_from(i).map_err(|_| "Invalid sparse tensor offset")?);
                }
            }
            [n, rank] if *n == count && *rank == dims.len() => {
                for row in indices.outer_iter() {
                    let mut offset = 0usize;
                    for (&index, &width) in row.iter().zip(&dims) {
                        let i = usize::try_from(index)
                            .ok()
                            .filter(|&i| i < width)
                            .ok_or("Invalid sparse tensor coordinate")?;
                        offset = offset
                            .checked_mul(width)
                            .and_then(|v| v.checked_add(i))
                            .ok_or("Sparse tensor offset overflow")?;
                    }
                    offsets.push(offset);
                }
            }
            _ => return Err("Invalid sparse tensor index shape".into()),
        }
        if offsets.windows(2).any(|v| v[0] >= v[1]) {
            return Err("Sparse tensor indices must be strictly increasing".into());
        }
        values.scatter(&dims, offsets.into_iter().map(Ok))
    }

    pub(super) fn scatter(
        &self,
        shape: &[usize],
        indices: impl Iterator<Item = Result<usize, String>>,
    ) -> Result<Self, String> {
        let count = super::window::volume(shape)?;
        dispatch!(self, |values, Kind| {
            let mut out = Vec::new();
            out.try_reserve_exact(count).map_err(|e| e.to_string())?;
            out.resize_with(count, Default::default);
            for (value, index) in values.iter().zip(indices) {
                *out.get_mut(index?)
                    .ok_or("ONNX index outside the output tensor")? = value.clone();
            }
            array(shape, out).map(Kind)
        })
    }

    pub(super) fn dtype(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::U8(_) => DataType::Uint8,
            Self::U16(_) => DataType::Uint16,
            Self::U32(_) => DataType::Uint32,
            Self::U64(_) => DataType::Uint64,
            Self::I8(_) => DataType::Int8,
            Self::I16(_) => DataType::Int16,
            Self::I32(_) => DataType::Int32,
            Self::I64(_) => DataType::Int64,
            Self::F16(_) => DataType::Float16,
            Self::F32(_) => DataType::Float,
            Self::F64(_) => DataType::Double,
            Self::String(_) => DataType::String,
        }
    }

    pub(super) fn shape(&self) -> &[usize] {
        macro_rules! shape {
            ($($ty:ident),+) => { match self {$(Tensor::$ty(a) => a.shape()),+} };
        }
        shape!(
            Bool, U8, U16, U32, U64, I8, I16, I32, I64, F16, F32, F64, String
        )
    }

    pub(super) fn reshape(&self, dims: &[usize]) -> Result<Self, String> {
        dispatch!(self, |a, Kind| {
            a.to_shape(IxDyn(dims))
                .map(|v| Kind(v.into_shared()))
                .map_err(|e| e.to_string())
        })
    }
}
