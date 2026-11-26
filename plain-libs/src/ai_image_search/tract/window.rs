use super::onnx::proto::NodeProto;

enum Padding {
    Explicit,
    Valid,
    SameUpper,
    SameLower,
}

pub(super) struct Window {
    pub kernel: Option<Vec<usize>>,
    strides: Option<Vec<usize>>,
    dilations: Option<Vec<usize>>,
    pads: Option<Vec<usize>>,
    auto_pad: Padding,
    ceil: bool,
}

pub(super) struct Geometry {
    pub output: Vec<usize>,
    before: Vec<usize>,
    strides: Vec<usize>,
    dilations: Vec<usize>,
}

pub(super) fn volume(dims: &[usize]) -> Result<usize, String> {
    dims.iter()
        .try_fold(1usize, |n, &d| n.checked_mul(d))
        .ok_or_else(|| "ONNX tensor volume overflow".into())
}

pub(super) fn coordinates(mut index: usize, shape: &[usize]) -> Vec<usize> {
    let mut coords = vec![0; shape.len()];
    for (i, &width) in shape.iter().enumerate().rev() {
        coords[i] = index % width;
        index /= width;
    }
    coords
}

impl Geometry {
    pub fn input_index(
        &self,
        output: &[usize],
        kernel: &[usize],
        input: &[usize],
    ) -> Option<usize> {
        let mut index = 0usize;
        for i in 0..input.len() {
            let padded = output[i]
                .checked_mul(self.strides[i])?
                .checked_add(kernel[i].checked_mul(self.dilations[i])?)?;
            let actual = padded.checked_sub(self.before[i])?;
            if actual >= input[i] {
                return None;
            }
            index = index * input[i] + actual;
        }
        Some(index)
    }
}

impl Window {
    pub fn parse(node: &NodeProto) -> Result<Self, String> {
        let dims = |name| -> Result<Option<Vec<usize>>, String> {
            node.axes(name)?
                .map(|v| {
                    v.into_iter()
                        .map(|n| {
                            usize::try_from(n)
                                .map_err(|_| format!("Invalid {} {name}", node.op_type))
                        })
                        .collect()
                })
                .transpose()
        };
        let auto_pad = match node
            .attribute
            .iter()
            .find(|a| a.name == "auto_pad")
            .map(|a| a.s.as_slice())
        {
            None | Some(b"NOTSET") => Padding::Explicit,
            Some(b"VALID") => Padding::Valid,
            Some(b"SAME_UPPER") => Padding::SameUpper,
            Some(b"SAME_LOWER") => Padding::SameLower,
            _ => return Err("Unknown ONNX auto_pad mode".into()),
        };
        Ok(Self {
            kernel: dims("kernel_shape")?,
            strides: dims("strides")?,
            dilations: dims("dilations")?,
            pads: dims("pads")?,
            auto_pad,
            ceil: node.integer("ceil_mode")?.unwrap_or(0) != 0,
        })
    }

    fn parameters(
        &self,
        rank: usize,
        kernel: &[usize],
    ) -> Result<(Vec<usize>, Vec<usize>, Vec<usize>), String> {
        let strides = self.strides.clone().unwrap_or_else(|| vec![1; rank]);
        let dilations = self.dilations.clone().unwrap_or_else(|| vec![1; rank]);
        let pads = self.pads.clone().unwrap_or_else(|| vec![0; rank * 2]);
        if rank == 0
            || self.kernel.as_deref().is_some_and(|k| k != kernel)
            || kernel.len() != rank
            || kernel.contains(&0)
            || strides.len() != rank
            || strides.contains(&0)
            || dilations.len() != rank
            || dilations.contains(&0)
            || pads.len() != 2 * rank
        {
            return Err("Invalid ONNX window shape, padding, strides or dilations".into());
        }
        Ok((strides, dilations, pads))
    }

    pub fn resolve(&self, spatial: &[usize], kernel: &[usize]) -> Result<Geometry, String> {
        let rank = spatial.len();
        let (strides, dilations, pads) = self.parameters(rank, kernel)?;
        let mut output = Vec::with_capacity(rank);
        let mut before = Vec::with_capacity(rank);
        for d in 0..rank {
            let width = spatial[d];
            let stride = strides[d];
            let effective = (kernel[d] - 1)
                .checked_mul(dilations[d])
                .and_then(|n| n.checked_add(1))
                .ok_or("ONNX kernel overflow")?;
            let (size, left) = match self.auto_pad {
                Padding::SameUpper | Padding::SameLower => {
                    let n = width.div_ceil(stride);
                    let total = n
                        .saturating_sub(1)
                        .checked_mul(stride)
                        .and_then(|v| v.checked_add(effective))
                        .ok_or("ONNX padding overflow")?
                        .saturating_sub(width);
                    (
                        n,
                        if matches!(self.auto_pad, Padding::SameUpper) {
                            total / 2
                        } else {
                            total.div_ceil(2)
                        },
                    )
                }
                Padding::Explicit | Padding::Valid => {
                    let (left, right) = if matches!(self.auto_pad, Padding::Valid) {
                        (0, 0)
                    } else {
                        (pads[d], pads[d + rank])
                    };
                    let padded = width
                        .checked_add(left)
                        .and_then(|v| v.checked_add(right))
                        .ok_or("ONNX extent overflow")?;
                    let mut n = if padded >= effective {
                        let extent = padded - effective;
                        1 + if self.ceil {
                            extent.div_ceil(stride)
                        } else {
                            extent / stride
                        }
                    } else {
                        usize::from(self.ceil && effective - padded < stride)
                    };
                    if self.ceil
                        && n > 0
                        && (n - 1).checked_mul(stride).ok_or("ONNX stride overflow")?
                            >= width + left
                    {
                        n -= 1;
                    }
                    (n, left)
                }
            };
            output.push(size);
            before.push(left);
        }
        Ok(Geometry {
            output,
            before,
            strides,
            dilations,
        })
    }

    pub fn unpool(&self, spatial: &[usize]) -> Result<Vec<usize>, String> {
        let kernel = self
            .kernel
            .as_ref()
            .ok_or("Missing MaxUnpool kernel shape")?;
        let (strides, _, pads) = self.parameters(spatial.len(), kernel)?;
        spatial
            .iter()
            .enumerate()
            .map(|(i, &n)| {
                let size = (n as i128 - 1) * strides[i] as i128 + kernel[i] as i128
                    - pads[i] as i128
                    - pads[i + spatial.len()] as i128;
                usize::try_from(size).map_err(|_| "Invalid MaxUnpool output extent".into())
            })
            .collect()
    }
}
