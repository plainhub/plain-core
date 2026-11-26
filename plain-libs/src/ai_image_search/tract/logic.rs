use super::{
    math::multi_broadcast,
    tensor::{Tensor, dispatch},
};
use crate::binary16::to_f32;
use ndarray::{IxDyn, Zip};

#[derive(Clone, Copy)]
pub(super) enum Comparison {
    Equal,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
}

impl Comparison {
    fn test<T: PartialOrd>(self, left: &T, right: &T) -> bool {
        match self {
            Self::Equal => left == right,
            Self::Less => left < right,
            Self::Greater => left > right,
            Self::LessEqual => left <= right,
            Self::GreaterEqual => left >= right,
        }
    }
    pub fn eval(self, left: &Tensor, right: &Tensor) -> Result<Tensor, String> {
        let shape = multi_broadcast(&[left.shape(), right.shape()])?;
        if let (Tensor::F16(a), Tensor::F16(b)) = (left, right) {
            let a = a
                .broadcast(IxDyn(&shape))
                .ok_or("Invalid comparison broadcast")?;
            let b = b
                .broadcast(IxDyn(&shape))
                .ok_or("Invalid comparison broadcast")?;
            return Ok(Tensor::Bool(
                Zip::from(a)
                    .and(b)
                    .map_collect(|&x, &y| self.test(&to_f32(x), &to_f32(y)))
                    .into_shared(),
            ));
        }
        if !matches!(self, Self::Equal) && matches!(left, Tensor::Bool(_) | Tensor::String(_)) {
            return Err("Expected numeric comparison inputs".into());
        }
        dispatch!(left, |a, Kind| {
            let Kind(b) = right else {
                return Err("ONNX comparison input types must match".into());
            };
            let a = a
                .broadcast(IxDyn(&shape))
                .ok_or("Invalid comparison broadcast")?;
            let b = b
                .broadcast(IxDyn(&shape))
                .ok_or("Invalid comparison broadcast")?;
            Ok(Tensor::Bool(
                Zip::from(a)
                    .and(b)
                    .map_collect(|x, y| self.test(x, y))
                    .into_shared(),
            ))
        })
    }
}

pub(super) enum Logic {
    And,
    Or,
    Xor,
}

impl Logic {
    pub fn eval(&self, left: &Tensor, right: &Tensor) -> Result<Tensor, String> {
        let (Tensor::Bool(a), Tensor::Bool(b)) = (left, right) else {
            return Err("Expected boolean ONNX inputs".into());
        };
        let shape = multi_broadcast(&[a.shape(), b.shape()])?;
        let left = a
            .broadcast(IxDyn(&shape))
            .ok_or("Invalid logical broadcast")?;
        let right = b
            .broadcast(IxDyn(&shape))
            .ok_or("Invalid logical broadcast")?;
        Ok(Tensor::Bool(
            Zip::from(left)
                .and(right)
                .map_collect(|&x, &y| match self {
                    Self::And => x && y,
                    Self::Or => x || y,
                    Self::Xor => x ^ y,
                })
                .into_shared(),
        ))
    }
}

pub(super) fn select(condition: &Tensor, yes: &Tensor, no: &Tensor) -> Result<Tensor, String> {
    let Tensor::Bool(condition) = condition else {
        return Err("Expected a boolean Where condition".into());
    };
    let shape = multi_broadcast(&[condition.shape(), yes.shape(), no.shape()])?;
    let condition = condition
        .broadcast(IxDyn(&shape))
        .ok_or("Invalid Where condition broadcast")?;
    dispatch!(yes, |a, Kind| {
        let Kind(b) = no else {
            return Err("Where value types must match".into());
        };
        let a = a
            .broadcast(IxDyn(&shape))
            .ok_or("Invalid Where value broadcast")?;
        let b = b
            .broadcast(IxDyn(&shape))
            .ok_or("Invalid Where value broadcast")?;
        Ok(Kind(
            Zip::from(condition)
                .and(a)
                .and(b)
                .map_collect(|&keep, x, y| if keep { x.clone() } else { y.clone() })
                .into_shared(),
        ))
    })
}
