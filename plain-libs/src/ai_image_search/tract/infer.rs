use super::onnx::proto::{ValueInfoProto, tensor_shape_proto::dimension, type_proto};
use super::{ops::Op, tensor::Tensor};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
enum Dimension {
    Unknown,
    Value(i64),
    Symbol(String),
}

#[derive(Clone, Default, Debug)]
pub(super) struct InferenceFact {
    element: Option<i32>,
    shape: Option<Vec<Dimension>>,
}

impl InferenceFact {
    pub(super) fn from_tensor(tensor: &Tensor) -> Self {
        Self {
            element: Some(tensor.dtype() as i32),
            shape: Some(
                tensor
                    .shape()
                    .iter()
                    .map(|&n| Dimension::Value(n as i64))
                    .collect(),
            ),
        }
    }

    pub(super) fn unify(&mut self, other: Self) -> Result<(), String> {
        if let Some(element) = other.element {
            if self.element.is_some_and(|known| known != element) {
                return Err("Conflicting ONNX element types".into());
            }
            self.element = Some(element);
        }
        let Some(inferred) = other.shape else {
            return Ok(());
        };
        let Some(declared) = &mut self.shape else {
            self.shape = Some(inferred);
            return Ok(());
        };
        if inferred.len() != declared.len() {
            return Err(format!(
                "Conflicting ONNX ranks {} and {}",
                inferred.len(),
                declared.len()
            ));
        }
        for (axis, inferred) in declared.iter_mut().zip(inferred) {
            match (&*axis, inferred) {
                (Dimension::Value(a), Dimension::Value(b)) if *a != b => {
                    return Err(format!("Conflicting ONNX dimensions {a} and {b}"));
                }
                (Dimension::Unknown, dim) | (Dimension::Symbol(_), dim @ Dimension::Value(_)) => {
                    *axis = dim
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn declared(value: &ValueInfoProto) -> Self {
        let Some(type_proto::Value::TensorType(tensor)) =
            value.r#type.as_ref().and_then(|t| t.value.as_ref())
        else {
            return Self::default();
        };
        let shape = tensor.shape.as_ref().map(|shape| {
            shape
                .dim
                .iter()
                .map(|d| match &d.value {
                    Some(dimension::Value::DimValue(n)) => Dimension::Value(*n),
                    Some(dimension::Value::DimParam(s)) if !s.is_empty() => {
                        Dimension::Symbol(s.clone())
                    }
                    _ => Dimension::Unknown,
                })
                .collect()
        });
        Self {
            element: (tensor.elem_type != 0).then_some(tensor.elem_type),
            shape,
        }
    }

    pub(super) fn set_symbols(&mut self, symbols: &HashMap<String, i64>) {
        for axis in self.shape.iter_mut().flatten() {
            if let Dimension::Symbol(s) = axis
                && let Some(&value) = symbols.get(s)
            {
                *axis = Dimension::Value(value);
            }
        }
    }

    pub(super) fn check(&self, dtype: i32, shape: &[usize]) -> Result<(), String> {
        if self.element.is_some_and(|n| n != dtype) {
            return Err(format!("Unexpected ONNX element type {dtype}"));
        }
        if let Some(dims) = &self.shape {
            if dims.len() != shape.len() || dims.iter().zip(shape).any(|(dim, &actual)| matches!(dim, Dimension::Value(n) if usize::try_from(*n) != Ok(actual))) {
                return Err(format!("Unexpected ONNX shape {shape:?}, expected {dims:?}"));
            }
        }
        Ok(())
    }
}

impl Op {
    pub(super) fn output_fact(
        &self,
        inputs: &[Option<usize>],
        facts: &[InferenceFact],
        output: usize,
    ) -> Option<InferenceFact> {
        let input = |i: usize| inputs.get(i).and_then(|slot| slot.map(|slot| &facts[slot]));
        match self {
            Self::Function(graph) => graph.output_fact(output).cloned(),
            Self::Const(tensor) => Some(InferenceFact::from_tensor(tensor)),
            Self::Identity
            | Self::Unary(_)
            | Self::LayerNorm(_)
            | Self::Softmax(..)
            | Self::Not => input(0).cloned(),
            Self::Cast(dtype) => Some(InferenceFact {
                element: Some(*dtype as i32),
                shape: input(0)?.shape.clone(),
            }),
            Self::CastLike => Some(InferenceFact {
                element: input(1)?.element,
                shape: input(0)?.shape.clone(),
            }),
            _ => None,
        }
    }
}
