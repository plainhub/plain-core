use super::array_ops::axis;
use super::math::{Binary, Unary, multi_broadcast};
use super::onnx::proto::{GraphProto, NodeProto, TensorProto, tensor_proto::DataType};
use super::ops::Op;
use super::reduce::Reducer;
use super::tensor::{Tensor, array};
use crate::binary16::{from_f32, to_f32};
use ndarray::{ArrayD, Axis};
use std::collections::HashMap;

pub(super) struct LayerNormalization {
    axis: i64,
    epsilon: f32,
    statistics: DataType,
    outputs: usize,
}

impl LayerNormalization {
    pub(super) fn from_node(node: &NodeProto) -> Result<Self, String> {
        if !(1..=3).contains(&node.output.len()) {
            return Err("Invalid LayerNormalization output count".into());
        }
        let statistics = match node
            .integer("stash_type")?
            .unwrap_or(DataType::Float as i64)
        {
            value if value == DataType::Float as i64 => DataType::Float,
            value if value == DataType::Bfloat16 as i64 => DataType::Bfloat16,
            _ => return Err("Invalid LayerNormalization stash_type".into()),
        };
        Ok(Self {
            axis: node.integer("axis")?.unwrap_or(-1),
            epsilon: node.float("epsilon")?.unwrap_or(1e-5),
            statistics,
            outputs: node.output.len(),
        })
    }

    pub(super) fn eval(
        &self,
        data: &Tensor,
        scale: &Tensor,
        bias: Option<&Tensor>,
    ) -> Result<Vec<Tensor>, String> {
        for parameter in std::iter::once(scale).chain(bias) {
            if parameter.dtype() != data.dtype()
                || multi_broadcast(&[data.shape(), parameter.shape()])? != data.shape()
            {
                return Err("LayerNormalization parameters must broadcast into the input".into());
            }
        }
        let start = axis(self.axis, data.shape().len(), false)?;
        let axes: Vec<_> = (start..data.shape().len())
            .map(|index| index as i64)
            .collect();
        let mean = |input: &Tensor| Reducer::Mean.eval(input, Some(&axes), true, false);
        let input = data.cast(self.statistics)?;
        let average = mean(&input)?;
        let centered = Binary::Sub.eval(&input, &average)?;
        let variance = mean(&Binary::Mul.eval(&centered, &centered)?)?;
        let epsilon = Tensor::F32(array(&[], vec![self.epsilon])?).cast(self.statistics)?;
        let deviation = Unary::Sqrt.eval(&Binary::Add.eval(&variance, &epsilon)?)?;
        let inverse = Unary::Reciprocal.eval(&deviation)?;
        let normalized = Binary::Mul.eval(&centered, &inverse)?;
        let mut result = Binary::Mul.eval(&normalized, &scale.cast(self.statistics)?)?;
        if let Some(bias) = bias {
            result = Binary::Add.eval(&result, &bias.cast(self.statistics)?)?;
        }
        let mut outputs = vec![result.cast(data.dtype())?];
        if self.outputs > 1 {
            outputs.push(average);
        }
        if self.outputs > 2 {
            outputs.push(inverse);
        }
        Ok(outputs)
    }
}

pub(super) struct LayerNorm {
    axis: i64,
    eps: f32,
}

impl LayerNorm {
    pub(super) fn eval(&self, input: &Tensor) -> Result<Tensor, String> {
        let at = axis(self.axis, input.shape().len(), false)?;
        let mut x: ArrayD<f32> = match input {
            Tensor::F16(a) => a.mapv(to_f32),
            Tensor::F32(a) => a.to_owned(),
            _ => return Err("Layer normalization requires float data".into()),
        };
        let width = x.shape()[at];
        if width != 0 {
            for mut row in x.lanes_mut(Axis(at)) {
                let mean = row.iter().copied().sum::<f32>() / width as f32;
                row.mapv_inplace(|v| v - mean);
                let inv = (row.iter().map(|v| v * v).sum::<f32>() / width as f32 + self.eps)
                    .sqrt()
                    .recip();
                row.mapv_inplace(|v| v * inv);
            }
        }
        Ok(if matches!(input, Tensor::F16(_)) {
            Tensor::F16(x.mapv(from_f32).into_shared())
        } else {
            Tensor::F32(x.into_shared())
        })
    }
}

struct Pattern<'a> {
    producers: HashMap<&'a str, &'a NodeProto>,
    constants: HashMap<&'a str, &'a TensorProto>,
}

impl<'a> Pattern<'a> {
    fn node(&self, name: &str, op: &str) -> Option<&'a NodeProto> {
        self.producers
            .get(name)
            .copied()
            .filter(|n| n.op_type == op && (n.domain.is_empty() || n.domain == "ai.onnx"))
    }
    fn scalar(&self, name: &str) -> Option<f32> {
        let proto = *self.constants.get(name)?;
        if !proto.dims.is_empty() {
            return None;
        }
        match Tensor::from_proto(proto.clone()).ok()? {
            Tensor::F32(a) => a.iter().next().copied(),
            Tensor::F16(a) => a.iter().next().copied().map(to_f32),
            _ => None,
        }
    }
    fn reduction(&self, name: &str) -> Option<(&'a str, i64)> {
        let node = self.node(name, "ReduceMean")?;
        if node.integer("keepdims").ok()?.unwrap_or(1) != 1 {
            return None;
        }
        let axes = if let Some(axes) = node.axes("axes").ok()? {
            axes
        } else {
            let proto = self.constants.get(node.input.get(1)?.as_str())?;
            if proto.data_type != DataType::Int64 as i32 || proto.dims.as_slice() != [1] {
                return None;
            }
            let Tensor::I64(a) = Tensor::from_proto((*proto).clone()).ok()? else {
                return None;
            };
            a.iter().copied().collect()
        };
        let [axis] = axes.as_slice() else {
            return None;
        };
        Some((node.input.first()?.as_str(), *axis))
    }
    fn match_layer_norm(&self, node: &NodeProto) -> Option<(String, LayerNorm)> {
        if node.op_type != "Div" || (!node.domain.is_empty() && node.domain != "ai.onnx") {
            return None;
        }
        let [centered, den] = node.input.as_slice() else {
            return None;
        };
        let [x, mean] = self.node(centered, "Sub")?.input.as_slice() else {
            return None;
        };
        let (source, axis) = self.reduction(mean)?;
        if source != x {
            return None;
        }
        let [sum] = self.node(den, "Sqrt")?.input.as_slice() else {
            return None;
        };
        let [l, r] = self.node(sum, "Add")?.input.as_slice() else {
            return None;
        };
        let (variance, eps) = if let Some(e) = self.scalar(l) {
            (r, e)
        } else {
            (l, self.scalar(r)?)
        };
        let (squared, variance_axis) = self.reduction(variance)?;
        if axis != variance_axis {
            return None;
        }
        if let Some(pow) = self.node(squared, "Pow") {
            let [base, exp] = pow.input.as_slice() else {
                return None;
            };
            if base != centered || self.scalar(exp)? != 2.0 {
                return None;
            }
        } else {
            let [a, b] = self.node(squared, "Mul")?.input.as_slice() else {
                return None;
            };
            if a != centered || b != centered {
                return None;
            }
        }
        Some((source.to_owned(), LayerNorm { axis, eps }))
    }
}

pub(super) fn detect(graph: &mut GraphProto, ops: &mut [Op], inputs: &[String]) {
    let pattern = Pattern {
        producers: graph
            .node
            .iter()
            .flat_map(|node| node.output.iter().map(move |v| (v.as_str(), node)))
            .collect(),
        constants: graph
            .initializer
            .iter()
            .filter(|tensor| !inputs.contains(&tensor.name))
            .map(|t| (t.name.as_str(), t))
            .collect(),
    };
    let changes: Vec<_> = graph
        .node
        .iter()
        .enumerate()
        .filter_map(|(i, node)| pattern.match_layer_norm(node).map(|v| (i, v)))
        .collect();
    for (index, (input, op)) in changes {
        graph.node[index].input = vec![input];
        ops[index] = Op::LayerNorm(op);
    }
}
