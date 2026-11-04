mod executable;
use crate::parser::pos::Positioned;
use crate::value::{ConstValue, Name, Value};

pub use executable::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, hash_map},
    fmt::{self, Display, Formatter, Write},
};
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}
impl Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Query => "query",
            Self::Mutation => "mutation",
            Self::Subscription => "subscription",
        })
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Type {
    pub base: BaseType,
    pub nullable: bool,
}
impl Type {
    #[must_use]
    pub fn new(ty: &str) -> Option<Self> {
        let (nullable, ty) = if let Some(rest) = ty.strip_suffix('!') {
            (false, rest)
        } else {
            (true, ty)
        };
        Some(Self {
            base: if let Some(ty) = ty.strip_prefix('[') {
                BaseType::List(Box::new(Self::new(ty.strip_suffix(']')?)?))
            } else {
                BaseType::Named(Name::new(ty))
            },
            nullable,
        })
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.base.fmt(f)?;
        if !self.nullable {
            f.write_char('!')?;
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum BaseType {
    Named(Name),
    List(Box<Type>),
}
impl Display for BaseType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name) => f.write_str(name),
            Self::List(ty) => write!(f, "[{}]", ty),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ConstDirective {
    pub name: Positioned<Name>,
    pub arguments: Vec<(Positioned<Name>, Positioned<ConstValue>)>,
}
impl ConstDirective {
    #[must_use]
    pub fn into_directive(self) -> Directive {
        Directive {
            name: self.name,
            arguments: self
                .arguments
                .into_iter()
                .map(|(name, value)| (name, value.map(ConstValue::into_value)))
                .collect(),
        }
    }
    #[must_use]
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<ConstValue>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directive {
    pub name: Positioned<Name>,
    pub arguments: Vec<(Positioned<Name>, Positioned<Value>)>,
}
impl Directive {
    #[must_use]
    pub fn into_const(self) -> Option<ConstDirective> {
        Some(ConstDirective {
            name: self.name,
            arguments: self
                .arguments
                .into_iter()
                .map(|(name, value)| {
                    Some((name, Positioned::new(value.node.into_const()?, value.pos)))
                })
                .collect::<Option<_>>()?,
        })
    }
    #[must_use]
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<Value>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}
