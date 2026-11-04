use crate::{
    Positioned,
    parser::types::VariableDefinition,
    validation::visitor::{Visitor, VisitorContext},
};
#[derive(Default)]
pub struct VariablesAreInputTypes;
impl<'a> Visitor<'a> for VariablesAreInputTypes {
    fn enter_variable_definition(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        variable_definition: &'a Positioned<VariableDefinition>,
    ) {
        if let Some(ty) = ctx
            .registry
            .concrete_type_by_parsed_type(&variable_definition.node.var_type.node)
            && !ty.is_input()
        {
            ctx.report_error(
                vec![variable_definition.pos],
                format!(
                    "Variable \"{}\" cannot be of non-input type \"{}\"",
                    variable_definition.node.name.node,
                    ty.name()
                ),
            );
        }
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/variables_are_input_types_tests.rs"]
mod tests;
