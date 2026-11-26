use super::onnx::proto::{NodeProto, attribute_proto::AttributeType};
use super::tensor::Tensor;
use ndarray::{Array4, ArrayView1, ArrayView2, ArrayView4};
use num_traits::Float;

#[derive(Clone, Copy)]
pub(super) enum Pooling {
    Average,
    Maximum,
}

pub(super) struct RoiAlign {
    pub output_height: usize,
    pub output_width: usize,
    pub sampling_ratio: usize,
    pub spatial_scale: f32,
    pub half_pixel: bool,
    pub pooling: Pooling,
}

impl RoiAlign {
    pub(super) fn from_node(node: &NodeProto, opset: i64) -> Result<Self, String> {
        if opset < 10 {
            return Err(format!("RoiAlign is undefined in ONNX opset {opset}"));
        }
        let mut op = RoiAlign {
            output_height: 1,
            output_width: 1,
            sampling_ratio: 0,
            spatial_scale: 1.0,
            half_pixel: opset >= 16,
            pooling: Pooling::Average,
        };
        for attr in &node.attribute {
            let dimension =
                || usize::try_from(attr.i).map_err(|_| format!("Invalid RoiAlign {}", attr.name));
            match (
                attr.name.as_str(),
                AttributeType::try_from(attr.r#type).ok(),
            ) {
                ("output_height", Some(AttributeType::Int)) => op.output_height = dimension()?,
                ("output_width", Some(AttributeType::Int)) => op.output_width = dimension()?,
                ("sampling_ratio", Some(AttributeType::Int)) => op.sampling_ratio = dimension()?,
                ("spatial_scale", Some(AttributeType::Float)) => op.spatial_scale = attr.f,
                ("mode", Some(AttributeType::String)) => {
                    op.pooling = match attr.s.as_slice() {
                        b"avg" => Pooling::Average,
                        b"max" => Pooling::Maximum,
                        _ => return Err("Unknown RoiAlign pooling mode".into()),
                    };
                }
                ("coordinate_transformation_mode", Some(AttributeType::String)) if opset >= 16 => {
                    op.half_pixel = match attr.s.as_slice() {
                        b"half_pixel" => true,
                        b"output_half_pixel" => false,
                        _ => return Err("Unknown RoiAlign coordinate transformation mode".into()),
                    };
                }
                _ => return Err(format!("Invalid RoiAlign attribute {}", attr.name)),
            }
        }
        Ok(op)
    }

    pub(super) fn evaluate(
        &self,
        pixels: &Tensor,
        boxes: &Tensor,
        batches: &Tensor,
    ) -> Result<Tensor, String> {
        let Tensor::I64(batches) = batches else {
            return Err("RoiAlign batch indices must be int64".into());
        };
        let batches = batches
            .view()
            .into_dimensionality()
            .map_err(|e| e.to_string())?;
        macro_rules! pool {
            ($image:expr, $boxes:expr, $kind:ident) => {{
                let image = $image
                    .view()
                    .into_dimensionality()
                    .map_err(|e| e.to_string())?;
                let boxes = $boxes
                    .view()
                    .into_dimensionality()
                    .map_err(|e| e.to_string())?;
                Ok(Tensor::$kind(
                    self.eval(image, boxes, batches)?.into_dyn().into_shared(),
                ))
            }};
        }
        match (pixels, boxes) {
            (Tensor::F32(image), Tensor::F32(boxes)) => pool!(image, boxes, F32),
            (Tensor::F64(image), Tensor::F64(boxes)) => pool!(image, boxes, F64),
            _ => Err("RoiAlign requires matching float image and region tensors".into()),
        }
    }

    pub(super) fn eval<T: Float>(
        &self,
        pixels: ArrayView4<'_, T>,
        boxes: ArrayView2<'_, T>,
        batches: ArrayView1<'_, i64>,
    ) -> Result<Array4<T>, String> {
        let (batch_count, channels, height, width) = pixels.dim();
        let regions = boxes.nrows();
        if boxes.ncols() != 4 || batches.len() != regions {
            return Err("RoiAlign expects boxes [K,4] and batch indices [K]".into());
        }
        if self.output_width == 0 || self.output_height == 0 || height == 0 || width == 0 {
            return Err("RoiAlign requires nonempty spatial dimensions".into());
        }
        let shape = (regions, channels, self.output_height, self.output_width);
        let count = [regions, channels, self.output_height, self.output_width]
            .into_iter()
            .try_fold(1usize, usize::checked_mul)
            .ok_or("RoiAlign output exceeds address space")?;
        let mut data = Vec::new();
        data.try_reserve_exact(count).map_err(|e| e.to_string())?;
        data.resize(count, T::zero());
        let mut output = Array4::from_shape_vec(shape, data).map_err(|e| e.to_string())?;
        let scale = T::from(self.spatial_scale).unwrap();
        let half = T::from(0.5).unwrap();
        let offset = if self.half_pixel { half } else { T::zero() };
        for (region, bounds) in boxes.outer_iter().enumerate() {
            let batch = usize::try_from(batches[region])
                .ok()
                .filter(|n| *n < batch_count)
                .ok_or("RoiAlign batch index out of bounds")?;
            let [left, top, right, bottom] = [0, 1, 2, 3].map(|i| bounds[i] * scale - offset);
            if [left, top, right, bottom].iter().any(|x| !x.is_finite()) {
                return Err("RoiAlign requires finite region coordinates".into());
            }
            let bin = |extent: T, outputs| {
                let extent = if self.half_pixel {
                    extent
                } else {
                    extent.max(T::one())
                };
                extent / T::from(outputs).unwrap()
            };
            let dy = bin(bottom - top, self.output_height);
            let dx = bin(right - left, self.output_width);
            let samples = |step: T| {
                if self.sampling_ratio == 0 {
                    step.ceil()
                        .max(T::zero())
                        .to_usize()
                        .ok_or("RoiAlign sampling grid exceeds address space")
                } else {
                    Ok(self.sampling_ratio)
                }
            };
            let ny = samples(dy)?;
            let nx = samples(dx)?;
            let samples_per_bin = ny
                .checked_mul(nx)
                .ok_or("RoiAlign sampling grid exceeds address space")?;
            if samples_per_bin == 0 {
                continue;
            }
            for y in 0..self.output_height {
                for x in 0..self.output_width {
                    for iy in 0..ny {
                        let sy = top
                            + T::from(y).unwrap() * dy
                            + (T::from(iy).unwrap() + half) * dy / T::from(ny).unwrap();
                        for ix in 0..nx {
                            let sx = left
                                + T::from(x).unwrap() * dx
                                + (T::from(ix).unwrap() + half) * dx / T::from(nx).unwrap();
                            let ys = neighbors(sy, height);
                            let xs = neighbors(sx, width);
                            for c in 0..channels {
                                let value = match (ys, xs) {
                                    (Some(ys), Some(xs)) => {
                                        let values = [
                                            ys[0].1
                                                * xs[0].1
                                                * pixels[[batch, c, ys[0].0, xs[0].0]],
                                            ys[0].1
                                                * xs[1].1
                                                * pixels[[batch, c, ys[0].0, xs[1].0]],
                                            ys[1].1
                                                * xs[0].1
                                                * pixels[[batch, c, ys[1].0, xs[0].0]],
                                            ys[1].1
                                                * xs[1].1
                                                * pixels[[batch, c, ys[1].0, xs[1].0]],
                                        ];
                                        match self.pooling {
                                            Pooling::Average => {
                                                values.into_iter().fold(T::zero(), |a, b| a + b)
                                            }
                                            Pooling::Maximum => {
                                                values.into_iter().fold(T::neg_infinity(), T::max)
                                            }
                                        }
                                    }
                                    _ => T::zero(),
                                };
                                let dst = &mut output[[region, c, y, x]];
                                *dst = match self.pooling {
                                    Pooling::Average => *dst + value,
                                    Pooling::Maximum if iy == 0 && ix == 0 => value,
                                    Pooling::Maximum => dst.max(value),
                                };
                            }
                        }
                    }
                    if matches!(self.pooling, Pooling::Average) {
                        for c in 0..channels {
                            let dst = &mut output[[region, c, y, x]];
                            *dst = *dst / T::from(samples_per_bin).unwrap();
                        }
                    }
                }
            }
        }
        Ok(output)
    }
}

fn neighbors<T: Float>(coordinate: T, size: usize) -> Option<[(usize, T); 2]> {
    if coordinate < -T::one() || coordinate > T::from(size).unwrap() {
        return None;
    }
    let coordinate = coordinate.max(T::zero()).min(T::from(size - 1).unwrap());
    let lo = coordinate.to_usize()?.min(size - 1);
    let fraction = coordinate - T::from(lo).unwrap();
    Some([
        (lo, T::one() - fraction),
        ((lo + 1).min(size - 1), fraction),
    ])
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/roi_align.rs"]
mod tests;
