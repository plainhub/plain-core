use crate::parser::{
    Error, Result,
    pos::{PositionCalculator, Positioned},
    types::*,
};
use pest::{
    Parser,
    iterators::{Pair, Pairs},
};
use std::collections::{HashMap, hash_map};
use utils::*;
mod executable;# [rustfmt :: skip] mod generated ;mod utils;
use crate::value::{ConstValue, Name, Number, Value};
pub use executable::parse_query;
use generated::Rule;
struct GraphQLParser;
fn parse_operation_type(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<OperationType>> {
    debug_assert_eq!(pair.as_rule(), Rule::OperationType);
    let pos = pc.step(&pair);
    Ok(Positioned::new(
        match pair.as_str() {
            "query" => OperationType::Query,
            "mutation" => OperationType::Mutation,
            "subscription" => OperationType::Subscription,
            _ => unreachable!(),
        },
        pos,
    ))
}
fn parse_default_value(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<ConstValue>> {
    debug_assert_eq!(pair.as_rule(), Rule::DefaultValue);
    parse_const_value(exactly_one(pair.into_inner()), pc)
}
fn parse_type(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Type>> {
    debug_assert_eq!(pair.as_rule(), Rule::Type);
    Ok(Positioned::new(
        Type::new(pair.as_str()).unwrap(),
        pc.step(&pair),
    ))
}
fn parse_const_value(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Positioned<ConstValue>> {
    debug_assert_eq!(pair.as_rule(), Rule::ConstValue);
    let pos = pc.step(&pair);
    let pair = exactly_one(pair.into_inner());
    Ok(Positioned::new(
        match pair.as_rule() {
            Rule::Number => ConstValue::Number(parse_number(pair, pc)?.node),
            Rule::String => ConstValue::String(parse_string(pair, pc)?.node),
            Rule::Boolean => ConstValue::Boolean(parse_boolean(pair, pc)?.node),
            Rule::Null => ConstValue::Null,
            Rule::EnumValue => ConstValue::Enum(parse_enum_value(pair, pc)?.node),
            Rule::ConstList => ConstValue::List(
                pair.into_inner()
                    .map(|pair| Ok(parse_const_value(pair, pc)?.node))
                    .collect::<Result<_>>()?,
            ),
            Rule::ConstObject => ConstValue::Object(
                pair.into_inner()
                    .map(|pair| {
                        debug_assert_eq!(pair.as_rule(), Rule::ConstObjectField);
                        let mut pairs = pair.into_inner();
                        let name = parse_name(pairs.next().unwrap(), pc)?;
                        let value = parse_const_value(pairs.next().unwrap(), pc)?;
                        debug_assert_eq!(pairs.next(), None);
                        Ok((name.node, value.node))
                    })
                    .collect::<Result<_>>()?,
            ),
            _ => unreachable!(),
        },
        pos,
    ))
}
fn parse_value(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Value>> {
    debug_assert_eq!(pair.as_rule(), Rule::Value);
    let pos = pc.step(&pair);
    let pair = exactly_one(pair.into_inner());
    Ok(Positioned::new(
        match pair.as_rule() {
            Rule::Variable => Value::Variable(parse_variable(pair, pc)?.node),
            Rule::Number => Value::Number(parse_number(pair, pc)?.node),
            Rule::String => Value::String(parse_string(pair, pc)?.node),
            Rule::Boolean => Value::Boolean(parse_boolean(pair, pc)?.node),
            Rule::Null => Value::Null,
            Rule::EnumValue => Value::Enum(parse_enum_value(pair, pc)?.node),
            Rule::List => Value::List(
                pair.into_inner()
                    .map(|pair| Ok(parse_value(pair, pc)?.node))
                    .collect::<Result<_>>()?,
            ),
            Rule::Object => Value::Object(
                pair.into_inner()
                    .map(|pair| {
                        debug_assert_eq!(pair.as_rule(), Rule::ObjectField);
                        let mut pairs = pair.into_inner();
                        let name = parse_name(pairs.next().unwrap(), pc)?;
                        let value = parse_value(pairs.next().unwrap(), pc)?;
                        debug_assert_eq!(pairs.next(), None);
                        Ok((name.node, value.node))
                    })
                    .collect::<Result<_>>()?,
            ),
            _ => unreachable!(),
        },
        pos,
    ))
}
fn parse_variable(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Name>> {
    debug_assert_eq!(pair.as_rule(), Rule::Variable);
    parse_name(exactly_one(pair.into_inner()), pc)
}
fn parse_number(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Number>> {
    debug_assert_eq!(pair.as_rule(), Rule::Number);
    let pos = pc.step(&pair);
    Ok(Positioned::new(
        pair.as_str().parse().map_err(|err| Error::Syntax {
            message: format!("invalid number: {}", err),
            start: pos,
            end: None,
        })?,
        pos,
    ))
}
fn parse_string(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<String>> {
    debug_assert_eq!(pair.as_rule(), Rule::String);
    let pos = pc.step(&pair);
    let pair = exactly_one(pair.into_inner());
    Ok(Positioned::new(
        match pair.as_rule() {
            Rule::BlockStringContent => block_string_value(pair.as_str()),
            Rule::StringContent => string_value(pair.as_str()),
            _ => unreachable!(),
        },
        pos,
    ))
}
fn parse_boolean(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<bool>> {
    debug_assert_eq!(pair.as_rule(), Rule::Boolean);
    let pos = pc.step(&pair);
    Ok(Positioned::new(
        match pair.as_str() {
            "true" => true,
            "false" => false,
            _ => unreachable!(),
        },
        pos,
    ))
}
fn parse_enum_value(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Name>> {
    debug_assert_eq!(pair.as_rule(), Rule::EnumValue);
    parse_name(exactly_one(pair.into_inner()), pc)
}
fn parse_opt_directives(
    pairs: &mut Pairs<'_, Rule>,
    pc: &mut PositionCalculator,
) -> Result<Vec<Positioned<Directive>>> {
    Ok(
        parse_if_rule(pairs, Rule::Directives, |pair| parse_directives(pair, pc))?
            .unwrap_or_default(),
    )
}
fn parse_directives(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Vec<Positioned<Directive>>> {
    debug_assert_eq!(pair.as_rule(), Rule::Directives);
    pair.into_inner()
        .map(|pair| parse_directive(pair, pc))
        .collect()
}
fn parse_directive(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Directive>> {
    debug_assert_eq!(pair.as_rule(), Rule::Directive);
    let pos = pc.step(&pair);
    let mut pairs = pair.into_inner();
    let name = parse_name(pairs.next().unwrap(), pc)?;
    let arguments = parse_if_rule(&mut pairs, Rule::Arguments, |pair| {
        parse_arguments(pair, pc)
    })?;
    debug_assert_eq!(pairs.next(), None);
    Ok(Positioned::new(
        Directive {
            name,
            arguments: arguments.unwrap_or_default(),
        },
        pos,
    ))
}
fn parse_arguments(
    pair: Pair<Rule>,
    pc: &mut PositionCalculator,
) -> Result<Vec<(Positioned<Name>, Positioned<Value>)>> {
    debug_assert_eq!(pair.as_rule(), Rule::Arguments);
    pair.into_inner()
        .map(|pair| {
            debug_assert_eq!(pair.as_rule(), Rule::Argument);
            let mut pairs = pair.into_inner();
            let name = parse_name(pairs.next().unwrap(), pc)?;
            let value = parse_value(pairs.next().unwrap(), pc)?;
            debug_assert_eq!(pairs.next(), None);
            Ok((name, value))
        })
        .collect()
}
fn parse_name(pair: Pair<Rule>, pc: &mut PositionCalculator) -> Result<Positioned<Name>> {
    debug_assert_eq!(pair.as_rule(), Rule::Name);
    Ok(Positioned::new(Name::new(pair.as_str()), pc.step(&pair)))
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/parser/parse/mod_tests.rs"]
mod tests;
