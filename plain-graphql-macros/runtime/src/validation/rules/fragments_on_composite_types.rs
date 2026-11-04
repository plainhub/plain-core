use crate::{
    Name, Positioned,
    parser::types::{FragmentDefinition, InlineFragment},
    validation::visitor::{Visitor, VisitorContext},
};
#[derive(Default)]
pub struct FragmentsOnCompositeTypes;
impl<'a> Visitor<'a> for FragmentsOnCompositeTypes {
    fn enter_fragment_definition(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        name: &'a Name,
        fragment_definition: &'a Positioned<FragmentDefinition>,
    ) {
        if let Some(current_type) = ctx.current_type()
            && !current_type.is_composite()
        {
            ctx.report_error(
                vec![fragment_definition.pos],
                format!(
                    "Fragment \"{}\" cannot condition non composite type \"{}\"",
                    name, fragment_definition.node.type_condition.node.on.node,
                ),
            );
        }
    }
    fn enter_inline_fragment(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        inline_fragment: &'a Positioned<InlineFragment>,
    ) {
        if let Some(current_type) = ctx.current_type()
            && !current_type.is_composite()
        {
            ctx.report_error(
                vec![inline_fragment.pos],
                format!(
                    "Fragment cannot condition non composite type \"{}\"",
                    current_type.name()
                ),
            );
        }
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/fragments_on_composite_types_tests.rs"]
mod tests;
