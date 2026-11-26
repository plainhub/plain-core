use super::array_ops::{
    concat, flatten, gather, integers, reshape, squeeze, tile, transpose, unsqueeze,
};
use super::logic::{Comparison, Logic};
use super::onnx::proto::{NodeProto, attribute_proto::AttributeType};
use super::roi_align::RoiAlign;
use super::tensor::Tensor;

use super::math::{Binary, Unary, softmax};

use super::reduce::Reducer;

pub(super) enum Op {
    Const(Tensor),
    Function(Box<super::model::Graph>),
    Control(Box<super::control_flow::ControlFlow>),
    LayerNorm(super::layer_norm::LayerNorm),
    LayerNormalization(super::layer_norm::LayerNormalization),
    MatMul,
    Conv(super::conv::Conv),
    Pool(super::pool::Pool),
    MaxUnpool(super::window::Window),
    Slice {
        starts: Option<Vec<i64>>,
        ends: Option<Vec<i64>>,
        axes: Option<Vec<i64>>,
    },
    Gemm {
        trans_a: bool,
        trans_b: bool,
        alpha: f32,
        beta: f32,
    },
    Reduce(Reducer, Option<Vec<i64>>, bool, bool),
    Binary(Binary),
    Nary(Binary, bool),
    Compare(Comparison),
    Logic(Logic),
    Not,
    Where,
    Expand,
    Size,
    CastLike,
    ConstantOfShape(Tensor),
    Unary(Unary),
    Softmax(i64, bool, bool),
    RoiAlign(RoiAlign),
    Flatten(i64),
    Concat(i64),
    Gather(i64),
    Identity,
    Cast(super::onnx::proto::tensor_proto::DataType),
    Shape {
        start: i64,
        end: Option<i64>,
    },
    Reshape(bool),
    Transpose(Option<Vec<i64>>),
    Squeeze(Option<Vec<i64>>),
    Unsqueeze(Option<Vec<i64>>),
    Tile,
}

impl NodeProto {
    pub(super) fn integer(&self, name: &str) -> Result<Option<i64>, String> {
        self.attribute
            .iter()
            .find(|a| a.name == name)
            .map(|attr| {
                if attr.r#type == AttributeType::Int as i32 {
                    Ok(attr.i)
                } else {
                    Err(format!("Expected an integer for {}.{name}", self.op_type))
                }
            })
            .transpose()
    }
    pub(super) fn float(&self, name: &str) -> Result<Option<f32>, String> {
        self.attribute
            .iter()
            .find(|a| a.name == name)
            .map(|attr| {
                if attr.r#type == AttributeType::Float as i32 {
                    Ok(attr.f)
                } else {
                    Err(format!("Expected a float for {}.{name}", self.op_type))
                }
            })
            .transpose()
    }

    pub(super) fn axes(&self, name: &str) -> Result<Option<Vec<i64>>, String> {
        self.attribute
            .iter()
            .find(|a| a.name == name)
            .map(|attr| {
                if attr.r#type == AttributeType::Ints as i32 {
                    Ok(attr.ints.clone())
                } else {
                    Err(format!(
                        "Expected a list of integers for {}.{name}",
                        self.op_type
                    ))
                }
            })
            .transpose()
    }
}

impl Op {
    pub(super) fn from_node(node: &NodeProto, opset: i64) -> Result<Self, String> {
        if !node.domain.is_empty() && node.domain != "ai.onnx" {
            return Err(format!("Unsupported ONNX domain {}", node.domain));
        }
        Ok(match node.op_type.as_str() {
            "Sum" => Self::Nary(Binary::Add, false),
            "Max" => Self::Nary(Binary::Max, false),
            "Min" => Self::Nary(Binary::Min, false),
            "Mean" => Self::Nary(Binary::Add, true),
            "Where" => Self::Where,
            "And" => Self::Logic(Logic::And),
            "Or" => Self::Logic(Logic::Or),
            "Xor" => Self::Logic(Logic::Xor),
            "Not" => Self::Not,
            "Equal" => Self::Compare(Comparison::Equal),
            "Greater" => Self::Compare(Comparison::Greater),
            "Less" => Self::Compare(Comparison::Less),
            "LessOrEqual" => Self::Compare(Comparison::LessEqual),
            "GreaterOrEqual" => Self::Compare(Comparison::GreaterEqual),
            "CastLike" => Self::CastLike,
            "Size" => Self::Size,
            "Expand" => Self::Expand,
            "ConstantOfShape" => {
                let scalar = if let Some(value) = node.attribute.iter().find(|a| a.name == "value")
                {
                    let value = Tensor::from_proto(
                        value
                            .t
                            .clone()
                            .ok_or("Expected ConstantOfShape value tensor")?,
                    )?;
                    if value.shape() != [1] {
                        return Err("Expected one ConstantOfShape element".into());
                    }
                    value.reshape(&[])?
                } else {
                    Tensor::F32(super::tensor::array(&[], vec![0.])?)
                };
                Self::ConstantOfShape(scalar)
            }
            "Add" => Self::Binary(Binary::Add),
            "Sub" => Self::Binary(Binary::Sub),
            "Mul" => Self::Binary(Binary::Mul),
            "Div" => Self::Binary(Binary::Div),
            "Pow" => Self::Binary(Binary::Pow),
            "Acos" => Self::Unary(Unary::Acos),
            "Acosh" => Self::Unary(Unary::Acosh),
            "Asin" => Self::Unary(Unary::Asin),
            "Asinh" => Self::Unary(Unary::Asinh),
            "Atan" => Self::Unary(Unary::Atan),
            "Atanh" => Self::Unary(Unary::Atanh),
            "Cosh" => Self::Unary(Unary::Cosh),
            "Sinh" => Self::Unary(Unary::Sinh),
            "Tan" => Self::Unary(Unary::Tan),
            "Round" => Self::Unary(Unary::Round),
            "Sign" => Self::Unary(Unary::Sign),
            "Erf" => Self::Unary(Unary::Erf),
            "HardSwish" => Self::Unary(Unary::HardSwish),
            "Softplus" => Self::Unary(Unary::Softplus),
            "Softsign" => Self::Unary(Unary::Softsign),
            "Mish" => Self::Unary(Unary::Mish),
            "Elu" => Self::Unary(Unary::Elu(node.float("alpha")?.unwrap_or(1.))),
            "Celu" => Self::Unary(Unary::Celu(node.float("alpha")?.unwrap_or(1.))),
            "LeakyRelu" => Self::Unary(Unary::LeakyRelu(node.float("alpha")?.unwrap_or(0.01))),
            "ThresholdedRelu" => {
                Self::Unary(Unary::ThresholdedRelu(node.float("alpha")?.unwrap_or(1.)))
            }
            "HardSigmoid" => Self::Unary(Unary::HardSigmoid(
                node.float("alpha")?.unwrap_or(0.2),
                node.float("beta")?.unwrap_or(0.5),
            )),
            "Selu" => Self::Unary(Unary::Selu(
                node.float("alpha")?.unwrap_or(1.6732632),
                node.float("gamma")?.unwrap_or(1.050701),
            )),
            "Gelu" => {
                let tanh = match node
                    .attribute
                    .iter()
                    .find(|a| a.name == "approximate")
                    .map(|a| a.s.as_slice())
                {
                    None | Some(b"none") => false,
                    Some(b"tanh") => true,
                    _ => return Err("Unknown Gelu approximation".into()),
                };
                Self::Unary(Unary::Gelu(tanh))
            }
            "Sqrt" => Self::Unary(Unary::Sqrt),
            "Tanh" => Self::Unary(Unary::Tanh),
            "Abs" => Self::Unary(Unary::Abs),
            "Neg" => Self::Unary(Unary::Neg),
            "Exp" => Self::Unary(Unary::Exp),
            "Log" => Self::Unary(Unary::Log),
            "Relu" => Self::Unary(Unary::Relu),
            "Reciprocal" => Self::Unary(Unary::Reciprocal),
            "Sigmoid" => Self::Unary(Unary::Sigmoid),
            "Sin" => Self::Unary(Unary::Sin),
            "Cos" => Self::Unary(Unary::Cos),
            "Floor" => Self::Unary(Unary::Floor),
            "Ceil" => Self::Unary(Unary::Ceil),
            "Softmax" | "LogSoftmax" => Self::Softmax(
                node.integer("axis")?
                    .unwrap_or(if opset < 13 { 1 } else { -1 }),
                opset < 13,
                node.op_type == "LogSoftmax",
            ),
            "LayerNormalization" => {
                Self::LayerNormalization(super::layer_norm::LayerNormalization::from_node(node)?)
            }
            "ReduceMean" | "ReduceSum" | "ReduceProd" | "ReduceMin" | "ReduceMax" | "ReduceL1"
            | "ReduceL2" | "ReduceSumSquare" => {
                let reduce = match node.op_type.as_str() {
                    "ReduceMean" => Reducer::Mean,
                    "ReduceSum" => Reducer::Sum,
                    "ReduceProd" => Reducer::Prod,
                    "ReduceMin" => Reducer::Min,
                    "ReduceL1" => Reducer::L1,
                    "ReduceL2" => Reducer::L2,
                    "ReduceSumSquare" => Reducer::SumSquare,
                    _ => Reducer::Max,
                };
                Self::Reduce(
                    reduce,
                    node.axes("axes")?,
                    node.integer("keepdims")?.unwrap_or(1) != 0,
                    node.integer("noop_with_empty_axes")?.unwrap_or(0) != 0,
                )
            }
            "RoiAlign" => Self::RoiAlign(RoiAlign::from_node(node, opset)?),
            "Flatten" => Self::Flatten(node.integer("axis")?.unwrap_or(1)),
            "Concat" => Self::Concat(node.integer("axis")?.ok_or("Missing Concat axis")?),
            "Gather" => Self::Gather(node.integer("axis")?.unwrap_or(0)),
            "Identity" => Self::Identity,
            "Constant" => {
                let attr = node
                    .attribute
                    .iter()
                    .find(|a| {
                        matches!(
                            a.name.as_str(),
                            "value"
                                | "sparse_value"
                                | "value_float"
                                | "value_floats"
                                | "value_int"
                                | "value_ints"
                                | "value_string"
                                | "value_strings"
                        )
                    })
                    .ok_or("Missing ONNX constant")?;
                let tensor = match attr.name.as_str() {
                    "sparse_value" => Tensor::from_sparse(
                        attr.sparse_tensor
                            .clone()
                            .ok_or("Missing sparse tensor constant")?,
                    )?,
                    "value" => {
                        Tensor::from_proto(attr.t.clone().ok_or("Missing tensor constant")?)?
                    }
                    "value_float" => Tensor::F32(super::tensor::array(&[], vec![attr.f])?),
                    "value_floats" => Tensor::F32(super::tensor::array(
                        &[attr.floats.len()],
                        attr.floats.clone(),
                    )?),
                    "value_int" => Tensor::I64(super::tensor::array(&[], vec![attr.i])?),
                    "value_ints" => {
                        Tensor::I64(super::tensor::array(&[attr.ints.len()], attr.ints.clone())?)
                    }
                    "value_string" => Tensor::String(super::tensor::array(
                        &[],
                        vec![std::sync::Arc::from(attr.s.clone())],
                    )?),
                    _ => Tensor::String(super::tensor::array(
                        &[attr.strings.len()],
                        attr.strings
                            .iter()
                            .cloned()
                            .map(std::sync::Arc::from)
                            .collect(),
                    )?),
                };
                Self::Const(tensor)
            }
            "MatMul" => Self::MatMul,
            "LpPool" | "AveragePool" | "MaxPool" | "GlobalLpPool" | "GlobalMaxPool"
            | "GlobalAveragePool" => Self::Pool(super::pool::Pool::parse(node)?),
            "MaxUnpool" => Self::MaxUnpool(super::window::Window::parse(node)?),
            "Conv" => Self::Conv(super::conv::Conv::parse(node)?),
            "Slice" => Self::Slice {
                starts: node.axes("starts")?,
                ends: node.axes("ends")?,
                axes: node.axes("axes")?,
            },
            "Gemm" => Self::Gemm {
                trans_a: node.integer("transA")?.unwrap_or(0) != 0,
                trans_b: node.integer("transB")?.unwrap_or(0) != 0,
                alpha: node.float("alpha")?.unwrap_or(1.),
                beta: node.float("beta")?.unwrap_or(1.),
            },
            "Cast" => Self::Cast(
                i32::try_from(node.integer("to")?.ok_or("Missing ONNX cast type")?)
                    .ok()
                    .and_then(|v| super::onnx::proto::tensor_proto::DataType::try_from(v).ok())
                    .ok_or("Invalid ONNX cast type")?,
            ),
            "Shape" => Self::Shape {
                start: node.integer("start")?.unwrap_or(0),
                end: node.integer("end")?,
            },
            "Reshape" => Self::Reshape(node.integer("allowzero")?.unwrap_or(0) != 0),
            "Transpose" => Self::Transpose(node.axes("perm")?),
            "Squeeze" => Self::Squeeze(node.axes("axes")?),
            "Unsqueeze" => Self::Unsqueeze(node.axes("axes")?),
            "Tile" => Self::Tile,
            _ => return Err(format!("Unsupported ONNX operator {}", node.op_type)),
        })
    }

    pub(super) fn eval(&self, inputs: &[Option<&Tensor>]) -> Result<Vec<Tensor>, String> {
        let result = match (self, inputs) {
            (Self::Pool(op), [Some(data)]) => return op.eval(data),
            (Self::MaxUnpool(window), [Some(data), Some(idx), ..]) if inputs.len() <= 3 => {
                super::pool::max_unpool(window, data, idx, inputs.get(2).copied().flatten())?
            }
            (Self::Function(graph), _) => {
                return graph.eval(inputs.iter().map(|v| v.cloned()).collect());
            }
            (Self::Control(op), _) => return op.eval(inputs),
            (Self::Const(value), []) => value.clone(),
            (Self::LayerNorm(norm), [Some(input)]) => norm.eval(input)?,
            (Self::LayerNormalization(norm), [Some(input), Some(scale), ..])
                if inputs.len() <= 3 =>
            {
                return norm.eval(input, scale, inputs.get(2).copied().flatten());
            }
            (Self::Conv(op), [Some(data), Some(w), ..]) if inputs.len() <= 3 => {
                op.eval(data, w, inputs.get(2).copied().flatten())?
            }

            (
                Self::Slice {
                    starts: Some(starts),
                    ends: Some(ends),
                    axes,
                },
                [Some(data)],
            ) => super::array_ops::slice(data, starts, ends, axes.as_deref(), None)?,
            (
                Self::Slice {
                    starts: None,
                    ends: None,
                    ..
                },
                [Some(data), Some(starts), Some(ends), ..],
            ) if inputs.len() <= 5 => {
                let axes = inputs.get(3).copied().flatten().map(integers).transpose()?;
                let steps = inputs.get(4).copied().flatten().map(integers).transpose()?;
                super::array_ops::slice(
                    data,
                    &integers(starts)?,
                    &integers(ends)?,
                    axes.as_deref(),
                    steps.as_deref(),
                )?
            }

            (Self::MatMul, [Some(a), Some(b)]) => super::matmul::matmul(a, b, false, false, 1.)?,
            (
                Self::Gemm {
                    trans_a,
                    trans_b,
                    alpha,
                    beta,
                },
                [Some(a), Some(b), ..],
            ) if inputs.len() <= 3 => {
                if a.shape().len() != 2 || b.shape().len() != 2 {
                    return Err("Gemm requires two matrices".into());
                }
                let product = super::matmul::matmul(a, b, *trans_a, *trans_b, *alpha)?;
                if let Some(Some(c)) = inputs.get(2) {
                    let term = if *beta == 1. {
                        (*c).clone()
                    } else {
                        let scalar =
                            Tensor::F32(super::tensor::array(&[], vec![*beta])?).cast(c.dtype())?;
                        Binary::Mul.eval(c, &scalar)?
                    };
                    Binary::Add.eval(&product, &term)?
                } else {
                    product
                }
            }

            (Self::Reduce(op, axes, keep, noop), [Some(data)] | [Some(data), None]) => {
                op.eval(data, axes.as_deref(), *keep, *noop)?
            }
            (Self::Reduce(op, None, keep, noop), [Some(data), Some(axes)]) => {
                op.eval(data, Some(&integers(axes)?), *keep, *noop)?
            }

            (Self::Cast(dt), [Some(a)]) => a.cast(*dt)?,
            (Self::Compare(op), [Some(a), Some(b)]) => op.eval(a, b)?,
            (Self::Logic(op), [Some(a), Some(b)]) => op.eval(a, b)?,
            (Self::Where, [Some(cond), Some(a), Some(b)]) => super::logic::select(cond, a, b)?,
            (Self::Not, [Some(Tensor::Bool(a))]) => Tensor::Bool(a.mapv(|x| !x).into_shared()),
            (Self::CastLike, [Some(a), Some(b)]) => a.cast(b.dtype())?,
            (Self::Size, [Some(data)]) => Tensor::I64(super::tensor::array(
                &[],
                vec![
                    i64::try_from(super::window::volume(data.shape())?)
                        .map_err(|_| "ONNX tensor size overflow")?,
                ],
            )?),
            (Self::Expand, [Some(data), Some(shape)]) => super::array_ops::expand(data, shape)?,
            (Self::ConstantOfShape(data), [Some(shape)]) => super::array_ops::expand(data, shape)?,
            (Self::Nary(op, mean), [Some(first), rest @ ..]) => {
                let mut result = (*first).clone();
                for input in rest {
                    result = op.eval(&result, input.ok_or("Missing ONNX reduction input")?)?;
                }
                if *mean {
                    let count = Tensor::I64(super::tensor::array(&[], vec![inputs.len() as i64])?)
                        .cast(result.dtype())?;
                    result = Binary::Div.eval(&result, &count)?;
                }
                result
            }
            (Self::Binary(op), [Some(a), Some(b)]) => op.eval(a, b)?,
            (Self::Unary(op), [Some(a)]) => op.eval(a)?,
            (Self::Softmax(axis, legacy, log), [Some(a)]) => softmax(a, *axis, *legacy, *log)?,

            (Self::RoiAlign(op), [Some(image), Some(boxes), Some(batches)]) => {
                op.evaluate(image, boxes, batches)?
            }
            (Self::Flatten(axis), [Some(data)]) => flatten(data, *axis)?,
            (Self::Concat(axis), _) => {
                let tensors = inputs
                    .iter()
                    .copied()
                    .collect::<Option<Vec<_>>>()
                    .ok_or("Missing Concat input")?;
                concat(&tensors, *axis)?
            }
            (Self::Gather(axis), [Some(data), Some(indices)]) => gather(data, indices, *axis)?,
            (Self::Identity, [Some(data)]) => (*data).clone(),
            (Self::Shape { start, end }, [Some(data)]) => {
                let rank = data.shape().len() as i64;
                let normalize =
                    |v: i64| if v < 0 { v.saturating_add(rank) } else { v }.clamp(0, rank) as usize;
                let start = normalize(*start);
                let end = normalize(end.unwrap_or(rank)).max(start);
                let dims = data.shape()[start..end]
                    .iter()
                    .map(|&n| n as i64)
                    .collect::<Vec<_>>();
                Tensor::I64(super::tensor::array(&[dims.len()], dims)?)
            }
            (Self::Reshape(allowzero), [Some(data), Some(shape)]) => {
                reshape(data, shape, *allowzero)?
            }
            (Self::Transpose(perm), [Some(data)]) => transpose(data, perm.as_deref())?,
            (Self::Squeeze(axes), [Some(data)] | [Some(data), None]) => {
                squeeze(data, axes.as_deref())?
            }
            (Self::Squeeze(None), [Some(data), Some(axes)]) => {
                squeeze(data, Some(&integers(axes)?))?
            }
            (Self::Unsqueeze(Some(axes)), [Some(data)]) => unsqueeze(data, axes)?,
            (Self::Unsqueeze(None), [Some(data), Some(axes)]) => unsqueeze(data, &integers(axes)?)?,
            (Self::Tile, [Some(data), Some(repeats)]) => tile(data, repeats)?,
            _ => return Err("Invalid ONNX input arity".into()),
        };
        Ok(vec![result])
    }
}
