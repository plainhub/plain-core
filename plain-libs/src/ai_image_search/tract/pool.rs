use super::{
    array_ops::integers,
    onnx::proto::NodeProto,
    tensor::{Array, Tensor, array},
    window::{Window, coordinates, volume},
};
use crate::binary16::{from_f32, to_f32};
use ndarray::ArrayViewD;
use num_traits::Float;

enum Reduction {
    Max { indices: bool, column_major: bool },
    Average { include_pad: bool },
    Lp(i32),
}

pub(super) struct Pool {
    window: Window,
    global: bool,
    reduction: Reduction,
}

impl Pool {
    pub fn parse(node: &NodeProto) -> Result<Self, String> {
        let global = node.op_type.starts_with("Global");
        let reduction = match node.op_type.trim_start_matches("Global") {
            "MaxPool" => Reduction::Max {
                indices: node.output.len() == 2,
                column_major: node.integer("storage_order")?.unwrap_or(0) == 1,
            },
            "AveragePool" => Reduction::Average {
                include_pad: node.integer("count_include_pad")?.unwrap_or(0) != 0,
            },
            "LpPool" => {
                let p = i32::try_from(node.integer("p")?.unwrap_or(2))
                    .map_err(|_| "Invalid LpPool exponent")?;
                if p <= 0 {
                    return Err("Invalid LpPool exponent".into());
                }
                Reduction::Lp(p)
            }
            _ => return Err("Unknown ONNX pooling operation".into()),
        };
        Ok(Self {
            window: Window::parse(node)?,
            global,
            reduction,
        })
    }

    fn pool<T: Float>(&self, x: ArrayViewD<'_, T>) -> Result<(Array<T>, Option<Tensor>), String> {
        if x.ndim() < 3 {
            return Err("Invalid pool input rank".into());
        }
        let spatial = &x.shape()[2..];
        let kernel = if self.global {
            spatial
        } else {
            self.window
                .kernel
                .as_deref()
                .ok_or("Missing pool kernel shape")?
        };
        let geometry = self.window.resolve(spatial, kernel)?;
        let mut shape = x.shape()[..2].to_vec();
        shape.extend(&geometry.output);
        let count = volume(&shape)?;
        let mut result = Vec::new();
        result.try_reserve_exact(count).map_err(|e| e.to_string())?;
        let mut indices = if matches!(self.reduction, Reduction::Max { indices: true, .. }) {
            let mut v = Vec::new();
            v.try_reserve_exact(count).map_err(|e| e.to_string())?;
            Some(v)
        } else {
            None
        };
        let input_size = volume(spatial)?;
        let output_size = volume(&geometry.output)?;
        let kernel_size = volume(kernel)?;
        let data = x.as_standard_layout();
        let data = data.as_slice().ok_or("Invalid pooling input layout")?;
        for outer in 0..volume(&x.shape()[..2])? {
            for output in 0..output_size {
                let position = coordinates(output, &geometry.output);
                let mut value = if matches!(self.reduction, Reduction::Max { .. }) {
                    T::neg_infinity()
                } else {
                    T::zero()
                };
                let mut max_index = None;
                let mut valid = 0usize;
                for k in 0..kernel_size {
                    if let Some(index) =
                        geometry.input_index(&position, &coordinates(k, kernel), spatial)
                    {
                        let current = data[outer * input_size + index];
                        match self.reduction {
                            Reduction::Max { .. } if max_index.is_none() || current > value => {
                                value = current;
                                max_index = Some(index);
                            }
                            Reduction::Max { .. } => {}
                            Reduction::Average { .. } => value = value + current,
                            Reduction::Lp(p) => value = value + current.abs().powi(p),
                        }
                        valid += 1;
                    }
                }
                result.push(match self.reduction {
                    Reduction::Max { column_major, .. } => {
                        if let Some(v) = &mut indices {
                            let index = max_index.unwrap_or(0);
                            let index = if column_major {
                                coordinates(index, spatial)
                                    .into_iter()
                                    .zip(spatial)
                                    .rev()
                                    .fold(0, |acc, (n, &width)| acc * width + n)
                            } else {
                                index
                            };
                            v.push(
                                i64::try_from(outer * input_size + index)
                                    .map_err(|_| "Pool index overflow")?,
                            );
                        }
                        value
                    }
                    Reduction::Average { include_pad } => {
                        value
                            / T::from(if include_pad { kernel_size } else { valid })
                                .ok_or("Invalid pool divisor")?
                    }
                    Reduction::Lp(p) => {
                        value.powf(T::one() / T::from(p).ok_or("Invalid LpPool exponent")?)
                    }
                });
            }
        }
        Ok((
            array(&shape, result)?,
            indices
                .map(|v| array(&shape, v).map(Tensor::I64))
                .transpose()?,
        ))
    }

    pub fn eval(&self, data: &Tensor) -> Result<Vec<Tensor>, String> {
        let (value, indices) = match data {
            Tensor::F32(x) => {
                let (v, i) = self.pool(x.view())?;
                (Tensor::F32(v), i)
            }
            Tensor::F64(x) => {
                let (v, i) = self.pool(x.view())?;
                (Tensor::F64(v), i)
            }
            Tensor::F16(x) => {
                let (v, i) = self.pool(x.mapv(to_f32).view())?;
                (Tensor::F16(v.mapv(from_f32).into_shared()), i)
            }
            Tensor::I8(x) if matches!(self.reduction, Reduction::Max { .. }) => {
                let (v, i) = self.pool(x.mapv(f32::from).view())?;
                (Tensor::I8(v.mapv(|n| n as i8).into_shared()), i)
            }
            Tensor::U8(x) if matches!(self.reduction, Reduction::Max { .. }) => {
                let (v, i) = self.pool(x.mapv(f32::from).view())?;
                (Tensor::U8(v.mapv(|n| n as u8).into_shared()), i)
            }
            _ => return Err("Unsupported ONNX pooling input type".into()),
        };
        let mut outputs = vec![value];
        outputs.extend(indices);
        Ok(outputs)
    }
}

pub(super) fn max_unpool(
    window: &Window,
    data: &Tensor,
    indices: &Tensor,
    output: Option<&Tensor>,
) -> Result<Tensor, String> {
    let Tensor::I64(indices) = indices else {
        return Err("Expected int64 MaxUnpool indices".into());
    };
    if data.shape().len() < 3 || data.shape() != indices.shape() {
        return Err("Invalid MaxUnpool input shape".into());
    }
    let mut shape = data.shape()[..2].to_vec();
    shape.extend(window.unpool(&data.shape()[2..])?);
    if let Some(output) = output {
        let dims = integers(output)?;
        if output.shape() != [shape.len()] {
            return Err("Invalid MaxUnpool output shape".into());
        }
        shape = dims
            .into_iter()
            .map(|d| usize::try_from(d).map_err(|_| "Invalid MaxUnpool extent"))
            .collect::<Result<_, _>>()?;
    }
    data.scatter(
        &shape,
        indices
            .iter()
            .map(|&i| usize::try_from(i).map_err(|_| "Invalid MaxUnpool index".into())),
    )
}
