use super::array_ops::{concat, transpose, unsqueeze};
use super::math::softmax;
use super::matmul::matmul;
use super::onnx::proto::NodeProto;
use super::tensor::{Array, Tensor, array};
use ndarray::{Axis, Ix2, Ix3, IxDyn};

pub(super) struct QAttention {
    heads: usize,
    causal: bool,
    scale: f32,
    mask_value: f32,
    present: bool,
    shared_past: bool,
}

impl QAttention {
    pub(super) fn from_node(node: &NodeProto) -> Result<Self, String> {
        let heads = node
            .integer("num_heads")?
            .and_then(|h| usize::try_from(h).ok())
            .filter(|&h| h > 0)
            .ok_or("QAttention requires a positive head count")?;
        if !(5..=9).contains(&node.input.len()) || !(1..=2).contains(&node.output.len()) {
            return Err("Invalid QAttention input or output count".into());
        }
        Ok(Self {
            heads,
            causal: node.integer("unidirectional")? == Some(1),
            scale: node.float("scale")?.unwrap_or(0.0),
            mask_value: node.float("mask_filter_value")?.unwrap_or(-10000.0),
            present: node.output.len() == 2,
            shared_past: node.integer("past_present_share_buffer")?.unwrap_or(0) != 0,
        })
    }

    pub(super) fn eval(&self, inputs: &[Option<&Tensor>]) -> Result<Vec<Tensor>, String> {
        let optional = |i: usize| inputs.get(i).copied().flatten();
        let (
            Some(Tensor::U8(input)),
            Some(weights),
            Some(Tensor::F32(bias)),
            Some(Tensor::F32(input_scale)),
            Some(Tensor::F32(weight_scale)),
        ) = (
            optional(0),
            optional(1),
            optional(2),
            optional(3),
            optional(4),
        )
        else {
            return Err("Invalid QAttention projection inputs".into());
        };
        let &[batch, length, depth] = input.shape() else {
            return Err("QAttention expects rank-three input".into());
        };
        let &[rows, columns] = weights.shape() else {
            return Err("QAttention expects a weight matrix".into());
        };
        let hidden = columns / 3;
        if depth != rows
            || columns % 3 != 0
            || hidden == 0
            || hidden % self.heads != 0
            || bias.shape() != [columns]
        {
            return Err("Invalid QAttention projection dimensions".into());
        }
        let head = hidden / self.heads;
        let past = match optional(8) {
            None => None,
            Some(Tensor::F32(p))
                if self.present
                    && !self.shared_past
                    && matches!(p.shape(), [2, b, n, _, h] if *b == batch && *n == self.heads && *h == head) =>
            {
                Some(p)
            }
            _ => return Err("Invalid QAttention past state".into()),
        };
        let prefix = past.map_or(0, |p| p.shape()[3]);
        let total = prefix
            .checked_add(length)
            .ok_or("QAttention sequence length overflow")?;
        let mask = Mask::new(optional(5), batch, length, total)?;
        let input_scale = scalar(input_scale)?;
        let input_zero = match optional(6) {
            None => 0,
            Some(Tensor::U8(z)) => scalar(z)?,
            _ => return Err("Invalid QAttention input zero point".into()),
        };
        let scale = columns_or_scalar(weight_scale, columns)?
            .map(|&s| s * input_scale)
            .collect::<Vec<_>>();
        let projected = match (weights, optional(7)) {
            (Tensor::I8(w), None) => project(input, w, input_zero, None, bias, &scale, self.heads)?,
            (Tensor::U8(w), None) => project(input, w, input_zero, None, bias, &scale, self.heads)?,
            (Tensor::I8(w), Some(Tensor::I8(z))) => {
                project(input, w, input_zero, Some(z), bias, &scale, self.heads)?
            }
            (Tensor::U8(w), Some(Tensor::U8(z))) => {
                project(input, w, input_zero, Some(z), bias, &scale, self.heads)?
            }
            _ => {
                return Err(
                    "QAttention weights and zero points must have matching 8-bit types".into(),
                );
            }
        };
        let part = |i| {
            Tensor::F32(
                projected
                    .clone()
                    .index_axis_move(Axis(2), i)
                    .permuted_axes(IxDyn(&[0, 2, 1, 3])),
            )
        };
        let state = |i| -> Result<Tensor, String> {
            let current = part(i + 1);
            if let Some(p) = past {
                let before = Tensor::F32(p.clone().index_axis_move(Axis(0), i));
                concat(&[&before, &current], 2)
            } else {
                Ok(current)
            }
        };
        let key = state(0)?;
        let value = state(1)?;
        let scale = if self.scale == 0.0 {
            1.0 / (head as f32).sqrt()
        } else {
            self.scale
        };
        let Tensor::F32(mut scores) = matmul(&part(0), &key, false, true, scale)? else {
            unreachable!()
        };
        if self.causal || !matches!(mask, Mask::None) {
            for (coord, score) in scores.indexed_iter_mut() {
                let (b, q, k) = (coord[0], coord[2], coord[3]);
                if self.causal && k > prefix + q {
                    *score += f32::MIN;
                } else if mask.hidden(b, q, k) {
                    *score += self.mask_value;
                }
            }
        }
        let probs = softmax(&Tensor::F32(scores), -1, false, false)?;
        let attended = matmul(&probs, &value, false, false, 1.0)?;
        let mut outputs =
            vec![transpose(&attended, Some(&[0, 2, 1, 3]))?.reshape(&[batch, length, hidden])?];
        if self.present {
            outputs.push(concat(
                &[&unsqueeze(&key, &[0])?, &unsqueeze(&value, &[0])?],
                0,
            )?);
        }
        Ok(outputs)
    }
}

fn scalar<T: Copy>(x: &Array<T>) -> Result<T, String> {
    if x.shape().is_empty() || x.shape() == [1] {
        Ok(*x.first().unwrap())
    } else {
        Err("QAttention expects a scalar or one-element vector".into())
    }
}

fn columns_or_scalar<T>(x: &Array<T>, columns: usize) -> Result<impl Iterator<Item = &T>, String> {
    if !x.shape().is_empty() && x.shape() != [1] && x.shape() != [columns] {
        return Err("Invalid QAttention per-column parameter".into());
    }
    Ok(x.iter().cycle().take(columns))
}

fn project<W: Copy + Into<i32>>(
    input: &Array<u8>,
    weight: &Array<W>,
    input_zero: u8,
    weight_zero: Option<&Array<W>>,
    bias: &Array<f32>,
    scale: &[f32],
    heads: usize,
) -> Result<Array<f32>, String> {
    let input = input
        .view()
        .into_dimensionality::<Ix3>()
        .map_err(|e| e.to_string())?;
    let weight = weight
        .view()
        .into_dimensionality::<Ix2>()
        .map_err(|e| e.to_string())?;
    let (batch, length, _) = input.dim();
    let columns = weight.ncols();
    let zero = weight_zero
        .map(|z| {
            columns_or_scalar(z, columns)
                .map(|values| values.copied().map(Into::into).collect::<Vec<i32>>())
        })
        .transpose()?;
    let count = batch
        .checked_mul(length)
        .and_then(|n| n.checked_mul(columns))
        .ok_or("QAttention projection size overflow")?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|e| e.to_string())?;
    let mut sums = vec![0i32; columns];
    for sample in input.outer_iter() {
        for row in sample.outer_iter() {
            sums.fill(0);
            let mut row_sum = 0i32;
            for (&v, w) in row.iter().zip(weight.rows()) {
                let v = i32::from(v) - i32::from(input_zero);
                row_sum = row_sum.wrapping_add(v);
                for (sum, &w) in sums.iter_mut().zip(w) {
                    *sum = sum.wrapping_add(v * w.into());
                }
            }
            if let Some(zero) = &zero {
                for (sum, &z) in sums.iter_mut().zip(zero) {
                    *sum = sum.wrapping_sub(row_sum.wrapping_mul(z));
                }
            }
            values.extend(
                sums.iter()
                    .zip(scale)
                    .zip(bias)
                    .map(|((&sum, &scale), &bias)| sum as f32 * scale + bias),
            );
        }
    }
    array(&[batch, length, 3, heads, columns / 3 / heads], values)
}

enum Mask<'a> {
    None,
    Range(&'a Array<i32>, Option<usize>),
    Keys(&'a Array<i32>),
    Matrix(&'a Array<i32>),
}

impl<'a> Mask<'a> {
    fn new(
        mask: Option<&'a Tensor>,
        batch: usize,
        length: usize,
        total: usize,
    ) -> Result<Self, String> {
        let Some(mask) = mask else {
            return Ok(Self::None);
        };
        let Tensor::I32(m) = mask else {
            return Err("Invalid QAttention mask type".into());
        };
        match m.shape() {
            [n] if *n == batch
                || Some(*n) == batch.checked_mul(2)
                || Some(*n) == batch.checked_mul(3).and_then(|b| b.checked_add(2)) =>
            {
                if m.iter().take(batch).any(|&end| end < 0) {
                    return Err("Negative QAttention mask end".into());
                }
                Ok(Self::Range(
                    m,
                    (Some(*n) == batch.checked_mul(2)).then_some(batch),
                ))
            }
            [b, t] if *b == batch && *t == total => Ok(Self::Keys(m)),
            [b, 1] if *b == 1 || *b == batch => Ok(Self::None),
            [b, s, t] if *b == batch && *s == length && *t == total => Ok(Self::Matrix(m)),
            _ => Err("Unsupported QAttention mask shape".into()),
        }
    }
    fn hidden(&self, b: usize, q: usize, k: usize) -> bool {
        match self {
            Self::None => false,
            Self::Range(m, start) => {
                k >= m[[b]] as usize || start.is_some_and(|n| k < m[[n + b]].max(0) as usize)
            }
            Self::Keys(m) => m[[b, k]] <= 0,
            Self::Matrix(m) => m[[b, q, k]] <= 0,
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/qattention.rs"]
mod tests;
