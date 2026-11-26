#[derive(Clone, PartialEq, prost::Message)]
pub struct AttributeProto {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "21")]
    pub ref_attr_name: String,
    #[prost(enumeration = "attribute_proto::AttributeType", tag = "20")]
    pub r#type: i32,
    #[prost(float, tag = "2")]
    pub f: f32,
    #[prost(int64, tag = "3")]
    pub i: i64,
    #[prost(bytes = "vec", tag = "4")]
    pub s: Vec<u8>,
    #[prost(message, optional, tag = "5")]
    pub t: Option<TensorProto>,
    #[prost(message, optional, tag = "6")]
    pub g: Option<GraphProto>,
    #[prost(message, optional, tag = "22")]
    pub sparse_tensor: Option<SparseTensorProto>,
    #[prost(float, repeated, tag = "7")]
    pub floats: Vec<f32>,
    #[prost(int64, repeated, tag = "8")]
    pub ints: Vec<i64>,
    #[prost(bytes = "vec", repeated, tag = "9")]
    pub strings: Vec<Vec<u8>>,
}
pub mod attribute_proto {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
    #[repr(i32)]
    pub enum AttributeType {
        Undefined = 0,
        Float = 1,
        Int = 2,
        String = 3,
        Tensor = 4,
        Graph = 5,
        SparseTensor = 11,
        TypeProto = 13,
        Floats = 6,
        Ints = 7,
        Strings = 8,
        Tensors = 9,
        Graphs = 10,
        SparseTensors = 12,
        TypeProtos = 14,
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct ValueInfoProto {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(message, optional, tag = "2")]
    pub r#type: Option<TypeProto>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct NodeProto {
    #[prost(string, repeated, tag = "1")]
    pub input: Vec<String>,
    #[prost(string, repeated, tag = "2")]
    pub output: Vec<String>,
    #[prost(string, tag = "4")]
    pub op_type: String,
    #[prost(string, tag = "8")]
    pub overload: String,
    #[prost(string, tag = "7")]
    pub domain: String,
    #[prost(message, repeated, tag = "5")]
    pub attribute: Vec<AttributeProto>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct ModelProto {
    #[prost(int64, optional, tag = "1")]
    pub ir_version: Option<i64>,
    #[prost(message, repeated, tag = "8")]
    pub opset_import: Vec<OperatorSetIdProto>,
    #[prost(message, optional, tag = "7")]
    pub graph: Option<GraphProto>,
    #[prost(message, repeated, tag = "25")]
    pub functions: Vec<FunctionProto>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct GraphProto {
    #[prost(message, repeated, tag = "1")]
    pub node: Vec<NodeProto>,
    #[prost(message, repeated, tag = "5")]
    pub initializer: Vec<TensorProto>,
    #[prost(message, repeated, tag = "15")]
    pub sparse_initializer: Vec<SparseTensorProto>,
    #[prost(message, repeated, tag = "11")]
    pub input: Vec<ValueInfoProto>,
    #[prost(message, repeated, tag = "12")]
    pub output: Vec<ValueInfoProto>,
    #[prost(message, repeated, tag = "13")]
    pub value_info: Vec<ValueInfoProto>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct TensorProto {
    #[prost(int64, repeated, tag = "1")]
    pub dims: Vec<i64>,
    #[prost(enumeration = "tensor_proto::DataType", tag = "2")]
    pub data_type: i32,
    #[prost(message, optional, tag = "3")]
    pub segment: Option<tensor_proto::Segment>,
    #[prost(float, repeated, tag = "4")]
    pub float_data: Vec<f32>,
    #[prost(int32, repeated, tag = "5")]
    pub int32_data: Vec<i32>,
    #[prost(bytes = "vec", repeated, tag = "6")]
    pub string_data: Vec<Vec<u8>>,
    #[prost(int64, repeated, tag = "7")]
    pub int64_data: Vec<i64>,
    #[prost(string, tag = "8")]
    pub name: String,
    #[prost(bytes = "bytes", tag = "9")]
    pub raw_data: prost::bytes::Bytes,
    #[prost(double, repeated, tag = "10")]
    pub double_data: Vec<f64>,
    #[prost(uint64, repeated, tag = "11")]
    pub uint64_data: Vec<u64>,
}
pub mod tensor_proto {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Segment {}
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
    #[repr(i32)]
    pub enum DataType {
        Undefined = 0,
        Float = 1,
        Uint8 = 2,
        Int8 = 3,
        Uint16 = 4,
        Int16 = 5,
        Int32 = 6,
        Int64 = 7,
        String = 8,
        Bool = 9,
        Float16 = 10,
        Double = 11,
        Uint32 = 12,
        Uint64 = 13,
        Complex64 = 14,
        Complex128 = 15,
        Bfloat16 = 16,
        Float8e4m3fn = 17,
        Float8e4m3fnuz = 18,
        Float8e5m2 = 19,
        Float8e5m2fnuz = 20,
        Uint4 = 21,
        Int4 = 22,
        Float4e2m1 = 23,
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct SparseTensorProto {
    #[prost(message, optional, tag = "1")]
    pub values: Option<TensorProto>,
    #[prost(message, optional, tag = "2")]
    pub indices: Option<TensorProto>,
    #[prost(int64, repeated, tag = "3")]
    pub dims: Vec<i64>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct TensorShapeProto {
    #[prost(message, repeated, tag = "1")]
    pub dim: Vec<tensor_shape_proto::Dimension>,
}
pub mod tensor_shape_proto {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Dimension {
        #[prost(oneof = "dimension::Value", tags = "1, 2")]
        pub value: Option<dimension::Value>,
    }
    pub mod dimension {
        #[derive(Clone, PartialEq, prost::Oneof)]
        pub enum Value {
            #[prost(int64, tag = "1")]
            DimValue(i64),
            #[prost(string, tag = "2")]
            DimParam(String),
        }
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct TypeProto {
    #[prost(oneof = "type_proto::Value", tags = "1")]
    pub value: Option<type_proto::Value>,
}
pub mod type_proto {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Tensor {
        #[prost(enumeration = "super::tensor_proto::DataType", tag = "1")]
        pub elem_type: i32,
        #[prost(message, optional, tag = "2")]
        pub shape: Option<super::TensorShapeProto>,
    }
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Value {
        #[prost(message, tag = "1")]
        TensorType(Tensor),
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct OperatorSetIdProto {
    #[prost(string, tag = "1")]
    pub domain: String,
    #[prost(int64, tag = "2")]
    pub version: i64,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct FunctionProto {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(string, repeated, tag = "4")]
    pub input: Vec<String>,
    #[prost(string, repeated, tag = "5")]
    pub output: Vec<String>,
    #[prost(string, repeated, tag = "6")]
    pub attribute: Vec<String>,
    #[prost(message, repeated, tag = "7")]
    pub node: Vec<NodeProto>,
    #[prost(message, repeated, tag = "9")]
    pub opset_import: Vec<OperatorSetIdProto>,
    #[prost(string, optional, tag = "10")]
    pub domain: Option<String>,
    #[prost(message, repeated, tag = "11")]
    pub attribute_proto: Vec<AttributeProto>,
    #[prost(message, repeated, tag = "12")]
    pub value_info: Vec<ValueInfoProto>,
    #[prost(string, tag = "13")]
    pub overload: String,
}
