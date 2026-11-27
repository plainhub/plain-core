use super::onnx::proto::{NodeProto, attribute_proto::AttributeType, tensor_proto::DataType};
use super::tensor::{Array, Tensor, array};
use ndarray::{Dimension, IxDyn, indices};
use std::collections::{BTreeMap, BTreeSet};

const ELLIPSIS: usize = usize::MAX;
const FIRST_ANONYMOUS_AXIS: usize = 128;

pub(super) struct EinSum {
    inputs: Vec<Vec<usize>>,
    output: Vec<usize>,
}

fn term(text: &str) -> Result<Vec<usize>, String> {
    let mut axes = Vec::new();
    let mut bytes = text.as_bytes();
    while let Some((&next, rest)) = bytes.split_first() {
        if next.is_ascii_alphabetic() {
            axes.push(usize::from(next));
            bytes = rest;
        } else if bytes.starts_with(b"...") && !axes.contains(&ELLIPSIS) {
            axes.push(ELLIPSIS);
            bytes = &bytes[3..];
        } else {
            return Err("Invalid Einsum subscript".into());
        }
    }
    Ok(axes)
}

impl EinSum {
    pub(super) fn from_node(node: &NodeProto, opset: i64) -> Result<Self, String> {
        let attr = node
            .attribute
            .iter()
            .find(|a| a.name == "equation" && a.r#type == AttributeType::String as i32)
            .ok_or("Missing Einsum equation")?;
        let equation = Self::parse(std::str::from_utf8(&attr.s).map_err(|e| e.to_string())?)?;
        if opset < 12 || equation.inputs.len() != node.input.len() || node.output.len() != 1 {
            return Err("Invalid Einsum opset or arity".into());
        }
        Ok(equation)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let text = text.replace(' ', "");
        let (input, output) = text
            .split_once("->")
            .map_or((text.as_str(), None), |(i, o)| (i, Some(o)));
        let inputs = input.split(',').map(term).collect::<Result<Vec<_>, _>>()?;
        let mut counts = BTreeMap::new();
        for &axis in inputs.iter().flatten() {
            *counts.entry(axis).or_insert(0usize) += 1;
        }
        let output = if let Some(output) = output {
            let axes = term(output)?;
            let mut seen = BTreeSet::new();
            if axes
                .iter()
                .any(|a| !counts.contains_key(a) || !seen.insert(*a))
            {
                return Err("Unknown or repeated Einsum output subscript".into());
            }
            axes
        } else {
            let mut axes = Vec::new();
            if counts.remove(&ELLIPSIS).is_some() {
                axes.push(ELLIPSIS);
            }
            axes.extend(
                counts
                    .into_iter()
                    .filter_map(|(axis, n)| (n == 1).then_some(axis)),
            );
            axes
        };
        Ok(Self { inputs, output })
    }

    pub(super) fn eval(&self, inputs: &[Option<&Tensor>]) -> Result<Tensor, String> {
        let inputs = inputs
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .ok_or("Missing Einsum input")?;
        let first = inputs.first().ok_or("Missing Einsum input")?;
        if self.inputs.len() != inputs.len() || inputs.iter().any(|t| t.dtype() != first.dtype()) {
            return Err("Einsum input count or element type mismatch".into());
        }
        let shapes: Vec<_> = inputs.iter().map(|t| t.shape()).collect();
        let plan = Plan::new(self, &shapes)?;
        macro_rules! evaluate {
            ($kind:ident, $add:expr, $mul:expr) => {{
                let arrays = inputs
                    .iter()
                    .map(|t| match t {
                        Tensor::$kind(a) => a,
                        _ => unreachable!(),
                    })
                    .collect::<Vec<_>>();
                Ok(Tensor::$kind(plan.run(&arrays, $add, $mul)?))
            }};
        }
        match first {
            Tensor::I8(_) => evaluate!(I8, i8::wrapping_add, i8::wrapping_mul),
            Tensor::I16(_) => evaluate!(I16, i16::wrapping_add, i16::wrapping_mul),
            Tensor::I32(_) => evaluate!(I32, i32::wrapping_add, i32::wrapping_mul),
            Tensor::I64(_) => evaluate!(I64, i64::wrapping_add, i64::wrapping_mul),
            Tensor::U8(_) => evaluate!(U8, u8::wrapping_add, u8::wrapping_mul),
            Tensor::U16(_) => evaluate!(U16, u16::wrapping_add, u16::wrapping_mul),
            Tensor::U32(_) => evaluate!(U32, u32::wrapping_add, u32::wrapping_mul),
            Tensor::U64(_) => evaluate!(U64, u64::wrapping_add, u64::wrapping_mul),
            Tensor::F32(_) => evaluate!(F32, std::ops::Add::add, std::ops::Mul::mul),
            Tensor::F64(_) => evaluate!(F64, std::ops::Add::add, std::ops::Mul::mul),
            Tensor::F16(_) => {
                let arrays = inputs
                    .iter()
                    .map(|t| match t.cast(DataType::Float)? {
                        Tensor::F32(a) => Ok(a),
                        _ => unreachable!(),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Tensor::F32(plan.run(
                    &arrays.iter().collect::<Vec<_>>(),
                    std::ops::Add::add,
                    std::ops::Mul::mul,
                )?)
                .cast(DataType::Float16)
            }
            _ => Err("Einsum requires numeric tensors".into()),
        }
    }
}

struct Plan {
    output: Vec<usize>,
    reduction: Vec<usize>,
    operand_axes: Vec<Vec<usize>>,
}

fn expand(axes: &[usize], width: usize, max_width: usize) -> Vec<usize> {
    let mut result = Vec::new();
    for &a in axes {
        if a == ELLIPSIS {
            result.extend((max_width - width..max_width).map(|i| FIRST_ANONYMOUS_AXIS + i));
        } else {
            result.push(a);
        }
    }
    result
}

impl Plan {
    fn new(equation: &EinSum, shapes: &[&[usize]]) -> Result<Self, String> {
        let mut widths = Vec::new();
        for (axes, shape) in equation.inputs.iter().zip(shapes) {
            let ellipsis = axes.contains(&ELLIPSIS);
            let named = axes.len() - usize::from(ellipsis);
            let width = shape
                .len()
                .checked_sub(named)
                .ok_or("Einsum operand rank too small")?;
            if !ellipsis && width != 0 {
                return Err("Einsum operand rank mismatch".into());
            }
            widths.push(width);
        }
        let max_width = widths.iter().copied().max().unwrap_or(0);
        let mut extents = BTreeMap::new();
        let mut operand_axes = Vec::new();
        for ((axes, width), &shape) in equation.inputs.iter().zip(widths).zip(shapes) {
            let axes = expand(axes, width, max_width);
            let mut diagonal = BTreeMap::new();
            for (&a, &n) in axes.iter().zip(shape) {
                if diagonal.insert(a, n).is_some_and(|d| d != n) {
                    return Err("Einsum diagonal dimensions differ".into());
                }
                let extent = extents.entry(a).or_insert(1);
                if *extent == 1 {
                    *extent = n;
                } else if n != 1 && n != *extent {
                    return Err("Einsum operand dimensions cannot broadcast".into());
                }
            }
            operand_axes.push(axes);
        }
        let output_axes = expand(&equation.output, max_width, max_width);
        let reduction_axes = extents
            .keys()
            .copied()
            .filter(|a| !output_axes.contains(a))
            .collect::<Vec<_>>();
        let positions: BTreeMap<_, _> = output_axes
            .iter()
            .chain(&reduction_axes)
            .copied()
            .enumerate()
            .map(|(i, a)| (a, i))
            .collect();
        for axes in &mut operand_axes {
            for a in axes {
                *a = positions[a];
            }
        }
        Ok(Self {
            output: output_axes.iter().map(|a| extents[a]).collect(),
            reduction: reduction_axes.iter().map(|a| extents[a]).collect(),
            operand_axes,
        })
    }

    fn run<T: Copy + num_traits::One + num_traits::Zero>(
        &self,
        arrays: &[&Array<T>],
        add: impl Fn(T, T) -> T,
        mul: impl Fn(T, T) -> T,
    ) -> Result<Array<T>, String> {
        let volume = |dims: &[usize]| {
            dims.iter()
                .filter(|&&d| d != 0)
                .try_fold(1isize, |n, &d| n.checked_mul(isize::try_from(d).ok()?))
                .ok_or("Einsum shape exceeds address space")
        };
        let size = volume(&self.output)?;
        volume(&self.reduction)?;
        let mut result = Vec::new();
        if !self.output.contains(&0) {
            result
                .try_reserve_exact(size as usize)
                .map_err(|e| e.to_string())?;
        }
        let mut coords = vec![0; self.output.len() + self.reduction.len()];
        let mut operand_coords: Vec<_> = arrays.iter().map(|a| IxDyn::zeros(a.ndim())).collect();
        for output in indices(IxDyn(&self.output)) {
            coords[..self.output.len()].copy_from_slice(output.slice());
            let mut sum = T::zero();
            for reduction in indices(IxDyn(&self.reduction)) {
                coords[self.output.len()..].copy_from_slice(reduction.slice());
                let mut product = T::one();
                for ((array, axes), index) in arrays
                    .iter()
                    .zip(&self.operand_axes)
                    .zip(&mut operand_coords)
                {
                    for ((coord, &axis), &size) in
                        index.slice_mut().iter_mut().zip(axes).zip(array.shape())
                    {
                        *coord = if size == 1 { 0 } else { coords[axis] };
                    }
                    product = mul(product, array[&*index]);
                }
                sum = add(sum, product);
            }
            result.push(sum);
        }
        array(&self.output, result)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/einsum.rs"]
mod tests;
