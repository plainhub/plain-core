use super::tensor::{Array, Tensor};
use crate::binary16::{from_f32, to_f32};
use ndarray::{ArrayViewD, IxDyn, Zip};
use num_traits::Float;

pub(super) fn multi_broadcast(shapes: &[&[usize]]) -> Result<Vec<usize>, String> {
    let rank = shapes.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut shape = Vec::with_capacity(rank);
    for axis in 0..rank {
        let mut extent = 1;
        for dims in shapes {
            let dim = dims.iter().rev().nth(axis).copied().unwrap_or(1);
            if extent == 1 {
                extent = dim;
            } else if dim != 1 && dim != extent {
                return Err(format!("Incompatible broadcast shapes {shapes:?}"));
            }
        }
        shape.push(extent);
    }
    shape.reverse();
    Ok(shape)
}

#[derive(Clone, Copy)]
pub(super) enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Min,
    Max,
}
impl Binary {
    fn float<T: Float>(self, a: T, b: T) -> T {
        match self {
            Self::Add => a + b,
            Self::Sub => a - b,
            Self::Mul => a * b,
            Self::Div => a / b,
            Self::Pow => a.powf(b),
            Self::Min => a.min(b),
            Self::Max => a.max(b),
        }
    }

    pub(super) fn eval(self, a: &Tensor, b: &Tensor) -> Result<Tensor, String> {
        let dims = multi_broadcast(&[a.shape(), b.shape()])?;
        macro_rules! binary {
            ($a:expr, $b:expr, $ty:ident, $func:expr) => {{
                let a = $a.broadcast(IxDyn(&dims)).ok_or("Invalid left broadcast")?;
                let b = $b
                    .broadcast(IxDyn(&dims))
                    .ok_or("Invalid right broadcast")?;
                Ok(Tensor::$ty(
                    Zip::from(a).and(b).map_collect($func).into_shared(),
                ))
            }};
        }
        macro_rules! int {
            ($a:expr, $b:expr, $ty:ident, $t:ty) => {{
                if matches!(self, Self::Div) && $b.iter().any(|&b| b == 0) {
                    return Err("Integer division by zero".into());
                }
                binary!($a, $b, $ty, |&x: &$t, &y: &$t| match self {
                    Self::Add => x.wrapping_add(y),
                    Self::Sub => x.wrapping_sub(y),
                    Self::Mul => x.wrapping_mul(y),
                    Self::Div => x.wrapping_div(y),
                    Self::Pow => x.wrapping_pow(y as u32),
                    Self::Min => x.min(y),
                    Self::Max => x.max(y),
                })
            }};
        }
        match (a, b) {
            (Tensor::F16(a), Tensor::F16(b)) => binary!(a, b, F16, |&x, &y| from_f32(
                self.float(to_f32(x), to_f32(y))
            )),
            (Tensor::F32(a), Tensor::F32(b)) => binary!(a, b, F32, |&x, &y| self.float(x, y)),
            (Tensor::F64(a), Tensor::F64(b)) => binary!(a, b, F64, |&x, &y| self.float(x, y)),
            (Tensor::I8(a), Tensor::I8(b)) => int!(a, b, I8, i8),
            (Tensor::I16(a), Tensor::I16(b)) => int!(a, b, I16, i16),
            (Tensor::I32(a), Tensor::I32(b)) => int!(a, b, I32, i32),
            (Tensor::I64(a), Tensor::I64(b)) => int!(a, b, I64, i64),
            (Tensor::U8(a), Tensor::U8(b)) => int!(a, b, U8, u8),
            (Tensor::U16(a), Tensor::U16(b)) => int!(a, b, U16, u16),
            (Tensor::U32(a), Tensor::U32(b)) => int!(a, b, U32, u32),
            (Tensor::U64(a), Tensor::U64(b)) => int!(a, b, U64, u64),
            _ => Err("Incompatible ONNX binary input types".into()),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Unary {
    Sqrt,
    Tanh,
    Abs,
    Neg,
    Exp,
    Log,
    Relu,
    Reciprocal,
    Sigmoid,
    Sin,
    Cos,
    Floor,
    Ceil,
    Acos,
    Acosh,
    Asin,
    Asinh,
    Atan,
    Atanh,
    Cosh,
    Sinh,
    Tan,
    Round,
    Sign,
    Erf,
    Elu(f32),
    Celu(f32),
    LeakyRelu(f32),
    ThresholdedRelu(f32),
    HardSigmoid(f32, f32),
    HardSwish,
    Softplus,
    Softsign,
    Mish,
    Selu(f32, f32),
    Gelu(bool),
}
impl Unary {
    fn float<T: Float>(self, x: T) -> T {
        let number = |n: f64| T::from(n).unwrap();
        let param = |n: f32| T::from(n).unwrap();
        let softplus = |x: T| x.max(T::zero()) + (-x.abs()).exp().ln_1p();
        match self {
            Self::Acos => x.acos(),
            Self::Acosh => x.acosh(),
            Self::Asin => x.asin(),
            Self::Asinh => x.asinh(),
            Self::Atan => x.atan(),
            Self::Atanh => x.atanh(),
            Self::Cosh => x.cosh(),
            Self::Sinh => x.sinh(),
            Self::Tan => x.tan(),
            Self::Round => {
                let rounded = x.round();
                if (x - rounded).abs() == number(0.5) && rounded % number(2.) != T::zero() {
                    (rounded - x.signum()).copysign(x)
                } else {
                    rounded
                }
            }
            Self::Sign => {
                if x.is_zero() {
                    x
                } else {
                    x.signum()
                }
            }
            Self::Erf => erf(x),
            Self::Elu(alpha) => {
                if x < T::zero() {
                    param(alpha) * x.exp_m1()
                } else {
                    x
                }
            }
            Self::Celu(alpha) => {
                if x < T::zero() {
                    param(alpha) * (x / param(alpha)).exp_m1()
                } else {
                    x
                }
            }
            Self::LeakyRelu(alpha) => {
                if x < T::zero() {
                    x * param(alpha)
                } else {
                    x
                }
            }
            Self::ThresholdedRelu(alpha) => {
                if x > param(alpha) {
                    x
                } else {
                    T::zero()
                }
            }
            Self::HardSigmoid(alpha, beta) => (param(alpha) * x + param(beta))
                .max(T::zero())
                .min(T::one()),
            Self::HardSwish => x * (x + number(3.)).max(T::zero()).min(number(6.)) / number(6.),
            Self::Softplus => softplus(x),
            Self::Softsign => x / (T::one() + x.abs()),
            Self::Mish => x * softplus(x).tanh(),
            Self::Selu(alpha, gamma) => {
                param(gamma)
                    * (if x < T::zero() {
                        param(alpha) * x.exp_m1()
                    } else {
                        x
                    })
            }
            Self::Gelu(tanh) => {
                let y = if tanh {
                    (number((2. / std::f64::consts::PI).sqrt())
                        * (x + number(0.044715) * x * x * x))
                        .tanh()
                } else {
                    erf(x * number(std::f64::consts::FRAC_1_SQRT_2))
                };
                number(0.5) * x * (T::one() + y)
            }
            Self::Sqrt => x.sqrt(),
            Self::Tanh => x.tanh(),
            Self::Abs => x.abs(),
            Self::Neg => -x,
            Self::Exp => x.exp(),
            Self::Log => x.ln(),
            Self::Relu => x.max(T::zero()),
            Self::Reciprocal => x.recip(),
            Self::Sigmoid => T::one() / (T::one() + (-x).exp()),
            Self::Sin => x.sin(),
            Self::Cos => x.cos(),
            Self::Floor => x.floor(),
            Self::Ceil => x.ceil(),
        }
    }

    pub(super) fn eval(self, input: &Tensor) -> Result<Tensor, String> {
        Ok(match input {
            Tensor::F16(a) => {
                Tensor::F16(a.mapv(|x| from_f32(self.float(to_f32(x)))).into_shared())
            }
            Tensor::F32(a) => Tensor::F32(a.mapv(|x| self.float(x)).into_shared()),
            Tensor::F64(a) => Tensor::F64(a.mapv(|x| self.float(x)).into_shared()),
            _ => return Err("Expected floating-point ONNX operand".into()),
        })
    }
}

fn erf<T: Float>(x: T) -> T {
    let a = x.abs();
    let y = [
        0.0000430638,
        0.0002765672,
        0.0001520143,
        0.0092705272,
        0.0422820123,
        0.0705230784,
    ]
    .into_iter()
    .fold(T::zero(), |v, c| (v + T::from(c).unwrap()) * a);
    (T::one() - (T::one() + y).powi(16).recip()).copysign(x)
}

pub(super) fn softmax(input: &Tensor, axis: i64, log: bool) -> Result<Tensor, String> {
    let rank = input.shape().len();
    let at = super::array_ops::axis(axis, rank, false)?;
    fn run<T: Float>(a: ArrayViewD<'_, T>, at: usize, log: bool) -> Array<T> {
        let mut result = a.to_owned();
        for mut lane in result.lanes_mut(ndarray::Axis(at)) {
            let max = lane.iter().copied().fold(T::neg_infinity(), T::max);
            lane.mapv_inplace(|x| x - max);
            let sum = lane.iter().fold(T::zero(), |s, &x| s + x.exp());
            if log {
                let log_sum = sum.ln();
                lane.mapv_inplace(|x| x - log_sum);
            } else {
                lane.mapv_inplace(|x| x.exp() / sum);
            }
        }
        result.into_shared()
    }
    Ok(match input {
        Tensor::F16(a) => Tensor::F16(
            run(a.mapv(to_f32).view(), at, log)
                .mapv(from_f32)
                .into_shared(),
        ),
        Tensor::F32(a) => Tensor::F32(run(a.view(), at, log)),
        Tensor::F64(a) => Tensor::F64(run(a.view(), at, log)),
        _ => return Err("Softmax requires float data".into()),
    })
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/math.rs"]
mod tests;
