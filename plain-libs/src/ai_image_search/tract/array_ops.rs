use super::tensor::{Tensor, checked_volume, dispatch};
use ndarray::{Axis, IxDyn};

pub(super) fn axis(index: i64, rank: usize, end: bool) -> Result<usize, String> {
    let n = if index < 0 {
        rank as i64 + index
    } else {
        index
    };
    usize::try_from(n)
        .ok()
        .filter(|&n| n < rank || end && n == rank)
        .ok_or_else(|| format!("Axis {index} outside rank {rank}"))
}

pub(super) fn flatten(data: &Tensor, at: i64) -> Result<Tensor, String> {
    let shape = data.shape();
    let at = axis(at, shape.len(), true)?;
    data.reshape(&[
        checked_volume(&shape[..at]).ok_or("Flatten shape overflow")?,
        checked_volume(&shape[at..]).ok_or("Flatten shape overflow")?,
    ])
}

pub(super) fn gather(data: &Tensor, indices: &Tensor, at: i64) -> Result<Tensor, String> {
    let at = axis(at, data.shape().len(), false)?;
    let width = data.shape()[at];
    let resolve = |i: i64| {
        let i = if i < 0 { i + width as i64 } else { i };
        usize::try_from(i)
            .ok()
            .filter(|&index| index < width)
            .ok_or_else(|| format!("Invalid gather index {i}"))
    };
    let resolved = match indices {
        Tensor::I64(values) => values
            .iter()
            .map(|&v| resolve(v))
            .collect::<Result<Vec<_>, _>>()?,
        Tensor::I32(values) => values
            .iter()
            .map(|&v| resolve(i64::from(v)))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("Gather indices must be int32 or int64".into()),
    };
    let mut shape = data.shape()[..at].to_vec();
    shape.extend(indices.shape());
    shape.extend(&data.shape()[at + 1..]);
    dispatch!(data, |a, Kind| {
        let a = a.select(Axis(at), &resolved);
        a.into_shape_with_order(IxDyn(&shape))
            .map(|v| Kind(v.into_shared()))
            .map_err(|e| e.to_string())
    })
}

pub(super) fn concat(inputs: &[&Tensor], at: i64) -> Result<Tensor, String> {
    let first = *inputs.first().ok_or("Concat requires an input")?;
    let at = axis(at, first.shape().len(), false)?;
    macro_rules! concat {
        ($($ty:ident),+) => {match first {
            $(Tensor::$ty(_) => {
                let arrays = inputs.iter().map(|input| match input {
                    Tensor::$ty(a) => Ok(a.view()),
                    _ => Err("Concat requires matching input types")
                }).collect::<Result<Vec<_>,_>>()?;
                ndarray::concatenate(Axis(at), &arrays).map(|a| Tensor::$ty(a.into_shared())).map_err(|e| e.to_string())
            }),+
        }};
    }
    concat!(
        Bool, U8, U16, U32, U64, I8, I16, I32, I64, F16, F32, F64, String
    )
}

pub(super) fn integers(input: &Tensor) -> Result<Vec<i64>, String> {
    match input {
        Tensor::I64(a) => Ok(a.iter().copied().collect()),
        Tensor::I32(a) => Ok(a.iter().map(|&v| i64::from(v)).collect()),
        _ => Err("Expected an integer shape tensor".into()),
    }
}

pub(super) fn reshape(
    data: &Tensor,
    requested: &Tensor,
    allow_zero: bool,
) -> Result<Tensor, String> {
    let mut inferred = None;
    let mut volume = 1usize;
    let mut shape = Vec::new();
    for (i, size) in integers(requested)?.into_iter().enumerate() {
        let dimension = match size {
            -1 if inferred.is_none() => {
                inferred = Some(i);
                1
            }
            0 if !allow_zero => *data.shape().get(i).ok_or("Invalid zero in reshape")?,
            n => usize::try_from(n).map_err(|_| "Invalid reshape dimension")?,
        };
        volume = volume
            .checked_mul(dimension)
            .ok_or("Reshape volume overflow")?;
        shape.push(dimension);
    }
    let elements = checked_volume(data.shape()).ok_or("Reshape input overflow")?;
    if let Some(i) = inferred {
        if volume == 0 || !elements.is_multiple_of(volume) {
            return Err("Cannot infer reshape dimension".into());
        }
        shape[i] = elements / volume;
    }
    data.reshape(&shape)
}

pub(super) fn transpose(data: &Tensor, perm: Option<&[i64]>) -> Result<Tensor, String> {
    let rank = data.shape().len();
    let axes = if let Some(perm) = perm {
        let axes: Vec<_> = perm
            .iter()
            .map(|&v| usize::try_from(v).map_err(|_| "Invalid axis permutation"))
            .collect::<Result<_, _>>()?;
        let mut sorted = axes.clone();
        sorted.sort_unstable();
        if !sorted.iter().copied().eq(0..rank) {
            return Err("Invalid axis permutation".into());
        }
        axes
    } else {
        (0..rank).rev().collect()
    };
    Ok(dispatch!(data, |a, Kind| Kind(
        a.clone().permuted_axes(IxDyn(&axes))
    )))
}

pub(super) fn squeeze(data: &Tensor, axes: Option<&[i64]>) -> Result<Tensor, String> {
    let dims = data.shape();
    let mut shape = Vec::new();
    if let Some(axes) = axes {
        let mut remove = vec![false; dims.len()];
        for &a in axes {
            let i = axis(a, dims.len(), false)?;
            if dims[i] != 1 || remove[i] {
                return Err("Invalid squeeze dimension".into());
            }
            remove[i] = true;
        }
        shape.extend(
            dims.iter()
                .zip(&remove)
                .filter_map(|(&n, &remove)| (!remove).then_some(n)),
        );
    } else {
        shape.extend(dims.iter().copied().filter(|&n| n != 1));
    }
    data.reshape(&shape)
}

pub(super) fn unsqueeze(data: &Tensor, axes: &[i64]) -> Result<Tensor, String> {
    let rank = data
        .shape()
        .len()
        .checked_add(axes.len())
        .ok_or("Invalid unsqueeze rank")?;
    let mut add = vec![false; rank];
    for &a in axes {
        let i = axis(a, rank, false)?;
        if std::mem::replace(&mut add[i], true) {
            return Err("Duplicate unsqueeze axis".into());
        }
    }
    let mut dims = data.shape().iter().copied();
    let shape: Vec<_> = add
        .iter()
        .map(|&add| if add { 1 } else { dims.next().unwrap() })
        .collect();
    data.reshape(&shape)
}

pub(super) fn tile(data: &Tensor, repeats: &Tensor) -> Result<Tensor, String> {
    use ndarray::{ArrayD, Dimension};
    let repeats = integers(repeats)?;
    if repeats.len() != data.shape().len() {
        return Err("Tile rank mismatch".into());
    }
    let shape: Vec<_> = data
        .shape()
        .iter()
        .zip(repeats)
        .map(|(&n, r)| {
            usize::try_from(r)
                .ok()
                .and_then(|r| n.checked_mul(r))
                .ok_or("Invalid tile extent")
        })
        .collect::<Result<_, _>>()?;
    checked_volume(&shape).ok_or("Tile volume overflow")?;
    Ok(dispatch!(data, |a, Kind| {
        Kind(
            ArrayD::from_shape_fn(IxDyn(&shape), |mut index| {
                for (i, &n) in index.slice_mut().iter_mut().zip(a.shape()) {
                    *i %= n;
                }
                a[index].clone()
            })
            .into_shared(),
        )
    }))
}

pub(super) fn slice(
    data: &Tensor,
    starts: &[i64],
    ends: &[i64],
    axes: Option<&[i64]>,
    steps: Option<&[i64]>,
) -> Result<Tensor, String> {
    if starts.len() != ends.len()
        || axes.is_some_and(|x| x.len() != starts.len())
        || steps.is_some_and(|x| x.len() != starts.len())
    {
        return Err("Slice parameters must have matching lengths".into());
    }
    let mut seen = vec![false; data.shape().len()];
    let mut result = data.clone();
    for (i, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        let at = axis(axes.map_or(i as i64, |a| a[i]), data.shape().len(), false)?;
        if std::mem::replace(&mut seen[at], true) {
            return Err("Duplicate Slice axis".into());
        }
        let size = i64::try_from(data.shape()[at]).map_err(|e| e.to_string())?;
        let step = steps.map_or(1, |v| v[i]);
        if step == 0 {
            return Err("Slice step must be nonzero".into());
        }
        let relative = |x: i64| if x < 0 { x.saturating_add(size) } else { x };
        let (mut index, limit) = if size == 0 {
            (0, 0)
        } else if step > 0 {
            (relative(start).clamp(0, size), relative(end).clamp(0, size))
        } else {
            (
                relative(start).clamp(0, size - 1),
                relative(end).clamp(-1, size - 1),
            )
        };
        let mut indices = Vec::new();
        while if step > 0 {
            index < limit
        } else {
            index > limit
        } {
            indices.push(index as usize);
            let Some(next) = index.checked_add(step) else {
                break;
            };
            index = next;
        }
        result = dispatch!(&result, |a, Kind| Kind(
            a.select(Axis(at), &indices).into_shared()
        ));
    }
    Ok(result)
}

pub(super) fn expand(data: &Tensor, shape: &Tensor) -> Result<Tensor, String> {
    if shape.shape().len() != 1 {
        return Err("Expected a rank-one expansion shape".into());
    }
    let requested = integers(shape)?
        .into_iter()
        .map(|i| usize::try_from(i).map_err(|_| "Invalid expansion extent"))
        .collect::<Result<Vec<_>, _>>()?;
    let dims = super::math::multi_broadcast(&[data.shape(), &requested])?;
    super::window::volume(&dims)?;
    dispatch!(data, |a, Kind| {
        let v = a.broadcast(IxDyn(&dims)).ok_or("Invalid expansion shape")?;
        Ok(Kind(v.to_shared()))
    })
}
