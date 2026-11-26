use super::{
    array_ops::axis,
    tensor::{Array, Tensor},
};
use crate::binary16::{from_f32, to_f32};
use ndarray::{ArrayViewD, Axis, IxDyn};
use num_traits::Float;

#[derive(Clone, Copy)]
pub(super) enum Reducer {
    Mean,
    Sum,
    Prod,
    Min,
    Max,
    L1,
    L2,
    SumSquare,
}

impl Reducer {
    fn reduce_float<T: Float>(
        self,
        input: ArrayViewD<'_, T>,
        axes: &[usize],
        keep: bool,
    ) -> Result<Array<T>, String> {
        let mut result = match self {
            Self::L1 => input.mapv(T::abs),
            Self::L2 | Self::SumSquare => input.mapv(|value| value * value),
            _ => input.to_owned(),
        };
        let mut count = 1usize;
        for &i in axes.iter().rev() {
            count = count
                .checked_mul(result.shape()[i])
                .ok_or("Reduction size overflow")?;
            result = result.map_axis(Axis(i), |lane| {
                let values = lane.iter().copied();
                match self {
                    Self::Mean | Self::Sum | Self::L1 | Self::L2 | Self::SumSquare => {
                        values.fold(T::zero(), |a, b| a + b)
                    }
                    Self::Prod => values.fold(T::one(), |a, b| a * b),
                    Self::Min => values.fold(T::infinity(), T::min),
                    Self::Max => values.fold(T::neg_infinity(), T::max),
                }
            });
        }
        if matches!(self, Self::Mean) {
            let count = T::from(count).ok_or("Invalid mean denominator")?;
            result.mapv_inplace(|x| x / count);
        } else if matches!(self, Self::L2) {
            result.mapv_inplace(T::sqrt);
        }
        let shape: Vec<_> = input
            .shape()
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| {
                if axes.contains(&i) {
                    keep.then_some(1)
                } else {
                    Some(n)
                }
            })
            .collect();
        result
            .into_shape_with_order(IxDyn(&shape))
            .map(|a| a.into_shared())
            .map_err(|e| e.to_string())
    }

    pub(super) fn eval(
        self,
        data: &Tensor,
        axes: Option<&[i64]>,
        keep: bool,
        noop: bool,
    ) -> Result<Tensor, String> {
        let rank = data.shape().len();
        let mut resolved = match axes {
            None | Some([]) if noop => Vec::new(),
            None | Some([]) => (0..rank).collect(),
            Some(axes) => axes
                .iter()
                .map(|&i| axis(i, rank, false))
                .collect::<Result<Vec<_>, _>>()?,
        };
        resolved.sort_unstable();
        if resolved.windows(2).any(|w| w[0] == w[1]) {
            return Err("Duplicate reduction axis".into());
        }
        Ok(match data {
            Tensor::F16(a) => Tensor::F16(
                self.reduce_float(a.mapv(to_f32).view(), &resolved, keep)?
                    .mapv(from_f32)
                    .into_shared(),
            ),
            Tensor::F32(a) => Tensor::F32(self.reduce_float(a.view(), &resolved, keep)?),
            Tensor::F64(a) => Tensor::F64(self.reduce_float(a.view(), &resolved, keep)?),
            _ => return Err("Expected a floating-point reduction input".into()),
        })
    }
}
