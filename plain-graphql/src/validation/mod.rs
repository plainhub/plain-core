pub(crate) mod enum_literals;
#[cfg(test)]
#[macro_use]
#[path = "../../../tests/plain-graphql/unit/validation/test_harness.rs"]
mod test_harness;
mod rules;
mod suggestion;
mod utils;
mod visitor;
mod visitors;
use crate::{
    CacheControl, ServerError, Variables, parser::types::ExecutableDocument, registry::Registry,
};
pub use visitor::VisitorContext;
use visitor::{VisitorNil, visit};
#[derive(Debug, Copy, Clone)]
pub struct ValidationResult {
    pub cache_control: CacheControl,
    pub complexity: usize,
    pub depth: usize,
}
pub(crate) fn check_rules(
    registry: &Registry,
    doc: &ExecutableDocument,
    variables: Option<&Variables>,
    operation_name: Option<&str>,
    limit_complexity: Option<usize>,
    limit_depth: Option<usize>,
) -> Result<ValidationResult, Vec<ServerError>> {
    let mut cache_control = CacheControl::default();
    let mut complexity = 0;
    let mut depth = 0;
    let mut ctx = VisitorContext::new(registry, doc, variables, operation_name);
    let errors = {
        let mut visitor = VisitorNil
            .with(rules::ArgumentsOfCorrectType::default())
            .with(rules::DefaultValuesOfCorrectType)
            .with(rules::FieldsOnCorrectType)
            .with(rules::FragmentsOnCompositeTypes)
            .with(rules::KnownArgumentNames::default())
            .with(rules::NoFragmentCycles::default())
            .with(rules::KnownFragmentNames)
            .with(rules::KnownTypeNames)
            .with(rules::NoUndefinedVariables::default())
            .with(rules::NoUnusedFragments::default())
            .with(rules::NoUnusedVariables::default())
            .with(rules::UniqueArgumentNames::default())
            .with(rules::UniqueVariableNames::default())
            .with(rules::VariablesAreInputTypes)
            .with(rules::VariableInAllowedPosition::default())
            .with(rules::ScalarLeafs)
            .with(rules::PossibleFragmentSpreads::default())
            .with(rules::ProvidedNonNullArguments)
            .with(rules::KnownDirectives::default())
            .with(rules::DirectivesUnique)
            .with(rules::OverlappingFieldsCanBeMerged);
        visit(&mut visitor, &mut ctx, doc);
        let mut visitor = VisitorNil
            .with(visitors::CacheControlCalculate {
                cache_control: &mut cache_control,
            })
            .with(visitors::ComplexityCalculate::new(&mut complexity))
            .with(visitors::DepthCalculate::new(&mut depth));
        visit(&mut visitor, &mut ctx, doc);
        ctx.errors
    };
    if let Some(limit_complexity) = limit_complexity
        && complexity > limit_complexity
    {
        return Err(vec![ServerError::new("Query is too complex.", None)]);
    }
    if let Some(limit_depth) = limit_depth
        && depth > limit_depth
    {
        return Err(vec![ServerError::new("Query is nested too deep.", None)]);
    }
    if !errors.is_empty() {
        return Err(errors.into_iter().map(Into::into).collect());
    }
    Ok(ValidationResult {
        cache_control,
        complexity,
        depth,
    })
}
