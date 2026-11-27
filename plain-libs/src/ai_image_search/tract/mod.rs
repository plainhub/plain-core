mod array_ops;
mod cast;
mod conv;
mod einsum;
mod infer;
mod layer_norm;
mod logic;
mod math;
mod matmul;
mod model;
mod onnx;
mod ops;
mod plan;
mod reduce;
mod tensor;
mod window;

use super::Input;
use std::path::Path;

pub(super) struct Session(model::Model);
impl Session {
    pub(super) fn new(path: &Path, inputs: &[Input<'_>], output: &str) -> Result<Self, String> {
        model::Model::open(path, inputs, output).map(Self)
    }
    pub(super) fn run(
        &mut self,
        inputs: &mut [Input<'_>],
        dimensions: usize,
    ) -> Result<Vec<f32>, String> {
        embedding(self.0.run(inputs)?, dimensions)
    }
}

fn embedding(output: tensor::Tensor, dimensions: usize) -> Result<Vec<f32>, String> {
    let tensor::Tensor::F32(output) = output else {
        return Err("Invalid ONNX embedding output type/shape".into());
    };
    if output.shape() != [1, dimensions] {
        return Err("Invalid ONNX embedding output type/shape".into());
    }
    let mut values = if output.is_standard_layout() {
        match output.try_into_owned_nocopy() {
            Ok(owned) => {
                let (mut data, offset) = owned.into_raw_vec_and_offset();
                let start = offset.unwrap_or(0);
                if start != 0 {
                    data.copy_within(start..start + dimensions, 0);
                }
                data.truncate(dimensions);
                data
            }
            Err(shared) => shared.iter().copied().collect(),
        }
    } else {
        output.iter().copied().collect()
    };
    super::embedding::normalize(&mut values)?;
    Ok(values)
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/embedding_output.rs"]
mod tests;
