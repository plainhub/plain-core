mod function;
pub(super) use function::ParsingContext;
pub(super) mod proto;

use crate::ai_image_search::{Input, Values};
use prost::{Message, bytes::Bytes};
use proto::{
    GraphProto, ModelProto, tensor_proto::DataType, tensor_shape_proto::dimension, type_proto,
};
use std::{collections::HashMap, path::Path};

pub(super) fn read(path: &Path) -> Result<ModelProto, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let model = ModelProto::decode(Bytes::from(bytes))
        .map_err(|error| format!("Invalid ONNX graph: {error}"))?;
    if model.opset_import.is_empty() {
        return Err("Missing opset in the model. All ModelProtos MUST have at least one entry that specifies which version of the ONNX OperatorSet is being imported.".into());
    }
    const MAX_IR_VERSION: i64 = 13;
    let ir_version = model.ir_version.ok_or("Missing model IR version.")?;
    if ir_version > MAX_IR_VERSION {
        return Err(format!(
            "Unsupported model IR version: {ir_version}, max supported IR version: {MAX_IR_VERSION}"
        ));
    }
    Ok(model)
}

pub(super) struct Bindings {
    pub input_order: Vec<usize>,
    pub output: usize,
    pub symbols: HashMap<String, i64>,
}

impl GraphProto {
    pub(super) fn bind(&self, inputs: &[Input<'_>], output: &str) -> Result<Bindings, String> {
        let output = self
            .output
            .iter()
            .position(|o| o.name == output)
            .ok_or_else(|| format!("Unknown encoder output {output}"))?;
        let mut symbols = HashMap::new();
        let mut input_order = Vec::with_capacity(inputs.len());
        for graph_input in &self.input {
            let name = &graph_input.name;
            let Some(slot) = inputs.iter().position(|input| input.name == name) else {
                if self.initializer.iter().any(|v| v.name == *name)
                    || self
                        .sparse_initializer
                        .iter()
                        .any(|v| v.values.as_ref().is_some_and(|v| v.name == *name))
                {
                    continue;
                }
                return Err(format!("Missing encoder input {name}"));
            };
            let input = &inputs[slot];
            let Some(type_proto::Value::TensorType(tensor)) =
                graph_input.r#type.as_ref().and_then(|t| t.value.as_ref())
            else {
                return Err(format!("Expected tensor type for encoder input {name}"));
            };
            let element = match input.values {
                Values::Float(_) => DataType::Float,
                Values::Integer(_) => DataType::Int64,
            };
            if tensor.elem_type != element as i32 {
                return Err(format!("Unexpected tensor type for encoder input {name}"));
            }
            if let Some(shape) = &tensor.shape {
                if shape.dim.len() != input.shape.len() {
                    return Err(format!("Unexpected input rank for {name}"));
                }
                for (given, axis) in input.shape.iter().zip(&shape.dim) {
                    match &axis.value {
                        Some(dimension::Value::DimValue(n)) if n != given => {
                            return Err(format!(
                                "Dimension mismatch for input {name}: expected {n}, got {given}"
                            ));
                        }
                        Some(dimension::Value::DimParam(s)) if !s.is_empty() => {
                            if symbols
                                .insert(s.clone(), *given)
                                .is_some_and(|n| n != *given)
                            {
                                return Err(format!("Inconsistent ONNX input dimension {s}"));
                            }
                        }
                        _ => {}
                    }
                }
            }
            input_order.push(slot);
        }
        if input_order.len() != inputs.len() {
            return Err("Encoder input names do not match the graph".into());
        }
        Ok(Bindings {
            input_order,
            output,
            symbols,
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/ai_image_search/tract/onnx.rs"]
mod tests;
