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
        let tensor::Tensor::F32(output) = self.0.run(inputs)? else {
            return Err("Invalid ONNX embedding output type/shape".into());
        };
        if output.shape() != [1, dimensions] {
            return Err("Invalid ONNX embedding output type/shape".into());
        }
        let mut values = output.iter().copied().collect::<Vec<_>>();
        super::embedding::normalize(&mut values)?;
        Ok(values)
    }
}
