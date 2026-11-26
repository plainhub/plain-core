use super::{
    onnx::proto::NodeProto,
    tensor::{Array, Tensor},
};
use crate::binary16::{from_f32, to_f32};
use ndarray::{Array2, ArrayView2, ArrayViewD, ArrayViewMut2, IxDyn, LinalgScalar};
use num_traits::Float;

use super::window::{Window, coordinates, volume};

pub(super) struct Conv {
    group: usize,
    window: Window,
}

impl Conv {
    pub(super) fn parse(node: &NodeProto) -> Result<Self, String> {
        let group = usize::try_from(node.integer("group")?.unwrap_or(1))
            .map_err(|_| "Invalid Conv group")?;
        Ok(Self {
            group,
            window: Window::parse(node)?,
        })
    }

    fn float<T: Float + LinalgScalar>(
        &self,
        data: ArrayViewD<'_, T>,
        weights: ArrayViewD<'_, T>,
        bias: Option<ArrayViewD<'_, T>>,
    ) -> Result<Array<T>, String> {
        if data.ndim() < 3 || weights.ndim() != data.ndim() {
            return Err("Conv input and kernel ranks must match".into());
        }
        let spatial = &data.shape()[2..];
        let kernel = &weights.shape()[2..];
        let (batch, channels) = (data.shape()[0], data.shape()[1]);
        let filters = weights.shape()[0];
        if self.group == 0
            || !channels.is_multiple_of(self.group)
            || !filters.is_multiple_of(self.group)
            || weights.shape()[1] != channels / self.group
            || bias.as_ref().is_some_and(|b| b.shape() != [filters])
        {
            return Err("Inconsistent Conv channels".into());
        }
        let geometry = self.window.resolve(spatial, kernel)?;
        let output = &geometry.output;
        let input_size = volume(spatial)?;
        let output_size = volume(output)?;
        let kernel_size = volume(kernel)?;
        let in_group = channels / self.group;
        let out_group = filters / self.group;
        let inner = kernel_size
            .checked_mul(in_group)
            .ok_or("Conv kernel overflow")?;
        let mut shape = vec![batch, filters];
        shape.extend(output);
        let count = volume(&shape)?;
        let mut result = Vec::new();
        result.try_reserve_exact(count).map_err(|e| e.to_string())?;
        result.resize(count, T::zero());
        if count == 0 {
            return super::tensor::array(&shape, result);
        }
        let pixels = data.as_standard_layout();
        let pixels = pixels.as_slice().ok_or("Invalid Conv input layout")?;
        let filters_data = weights.as_standard_layout();
        let filters_data = filters_data
            .as_slice()
            .ok_or("Invalid Conv weight layout")?;
        let bias = bias.map(|b| b.iter().copied().collect::<Vec<_>>());
        let kernel_coordinates = (0..kernel_size)
            .map(|k| coordinates(k, kernel))
            .collect::<Vec<_>>();
        for b in 0..batch {
            for g in 0..self.group {
                let first = g * out_group;
                let w = ArrayView2::from_shape(
                    (out_group, inner),
                    &filters_data[first * inner..(first + out_group) * inner],
                )
                .map_err(|e| e.to_string())?;
                for offset in (0..output_size).step_by(128) {
                    let block = (output_size - offset).min(128);
                    let mut columns = Array2::zeros((inner, block));
                    for col in 0..block {
                        let pos = coordinates(offset + col, output);
                        for (k, point) in kernel_coordinates.iter().enumerate() {
                            if let Some(index) = geometry.input_index(&pos, point, spatial) {
                                for c in 0..in_group {
                                    columns[[c * kernel_size + k, col]] = pixels
                                        [((b * channels + g * in_group + c) * input_size) + index];
                                }
                            }
                        }
                    }
                    let mut product = Array2::zeros((out_group, block));
                    ndarray::linalg::general_mat_mul(
                        T::one(),
                        &w,
                        &columns,
                        T::zero(),
                        &mut product,
                    );
                    let mut target = ArrayViewMut2::from_shape(
                        (out_group, output_size),
                        &mut result[(b * filters + first) * output_size
                            ..(b * filters + first + out_group) * output_size],
                    )
                    .map_err(|e| e.to_string())?;
                    for filter in 0..out_group {
                        let add = bias.as_ref().map_or(T::zero(), |v| v[first + filter]);
                        for col in 0..block {
                            target[[filter, offset + col]] = product[[filter, col]] + add;
                        }
                    }
                }
            }
        }
        Ok(ndarray::ArrayD::from_shape_vec(IxDyn(&shape), result)
            .map_err(|e| e.to_string())?
            .into_shared())
    }

    pub(super) fn eval(
        &self,
        x: &Tensor,
        w: &Tensor,
        b: Option<&Tensor>,
    ) -> Result<Tensor, String> {
        Ok(match (x, w, b) {
            (Tensor::F32(x), Tensor::F32(w), None) => {
                Tensor::F32(self.float(x.view(), w.view(), None)?)
            }
            (Tensor::F32(x), Tensor::F32(w), Some(Tensor::F32(b))) => {
                Tensor::F32(self.float(x.view(), w.view(), Some(b.view()))?)
            }
            (Tensor::F64(x), Tensor::F64(w), None) => {
                Tensor::F64(self.float(x.view(), w.view(), None)?)
            }
            (Tensor::F64(x), Tensor::F64(w), Some(Tensor::F64(b))) => {
                Tensor::F64(self.float(x.view(), w.view(), Some(b.view()))?)
            }
            (Tensor::F16(x), Tensor::F16(w), bias) => {
                let b = match bias {
                    None => None,
                    Some(Tensor::F16(b)) => Some(b.mapv(to_f32)),
                    _ => return Err("Conv bias type mismatch".into()),
                };
                Tensor::F16(
                    self.float(
                        x.mapv(to_f32).view(),
                        w.mapv(to_f32).view(),
                        b.as_ref().map(|b| b.view()),
                    )?
                    .mapv(from_f32)
                    .into_shared(),
                )
            }
            _ => return Err("Inconsistent Conv input types".into()),
        })
    }
}
