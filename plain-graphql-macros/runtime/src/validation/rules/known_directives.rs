use crate::{
    Name, Positioned,
    model::__DirectiveLocation,
    parser::types::{
        Directive, Field, FragmentDefinition, FragmentSpread, InlineFragment, OperationDefinition,
        OperationType,
    },
    validation::visitor::{Visitor, VisitorContext},
};
#[derive(Default)]
pub struct KnownDirectives {
    location_stack: Vec<__DirectiveLocation>,
}
impl<'a> Visitor<'a> for KnownDirectives {
    fn enter_operation_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _name: Option<&'a Name>,
        operation_definition: &'a Positioned<OperationDefinition>,
    ) {
        self.location_stack
            .push(match &operation_definition.node.ty {
                OperationType::Query => __DirectiveLocation::Query,
                OperationType::Mutation => __DirectiveLocation::Mutation,
                OperationType::Subscription => __DirectiveLocation::Subscription,
            });
    }
    fn exit_operation_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _name: Option<&'a Name>,
        _operation_definition: &'a Positioned<OperationDefinition>,
    ) {
        self.location_stack.pop();
    }
    fn enter_fragment_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _name: &'a Name,
        _fragment_definition: &'a Positioned<FragmentDefinition>,
    ) {
        self.location_stack
            .push(__DirectiveLocation::FragmentDefinition);
    }
    fn exit_fragment_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _name: &'a Name,
        _fragment_definition: &'a Positioned<FragmentDefinition>,
    ) {
        self.location_stack.pop();
    }
    fn enter_directive(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        directive: &'a Positioned<Directive>,
    ) {
        if let Some(schema_directive) = ctx
            .registry
            .directives
            .get(directive.node.name.node.as_str())
        {
            if let Some(current_location) = self.location_stack.last()
                && !schema_directive.locations.contains(current_location)
            {
                ctx.report_error(
                    vec![directive.pos],
                    format!(
                        "Directive \"{}\" may not be used on \"{:?}\"",
                        directive.node.name.node, current_location
                    ),
                )
            }
        } else {
            ctx.report_error(
                vec![directive.pos],
                format!("Unknown directive \"{}\"", directive.node.name.node),
            );
        }
    }
    fn enter_field(&mut self, _ctx: &mut VisitorContext<'a>, _field: &'a Positioned<Field>) {
        self.location_stack.push(__DirectiveLocation::Field);
    }
    fn exit_field(&mut self, _ctx: &mut VisitorContext<'a>, _field: &'a Positioned<Field>) {
        self.location_stack.pop();
    }
    fn enter_fragment_spread(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _fragment_spread: &'a Positioned<FragmentSpread>,
    ) {
        self.location_stack
            .push(__DirectiveLocation::FragmentSpread);
    }
    fn exit_fragment_spread(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _fragment_spread: &'a Positioned<FragmentSpread>,
    ) {
        self.location_stack.pop();
    }
    fn enter_inline_fragment(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _inline_fragment: &'a Positioned<InlineFragment>,
    ) {
        self.location_stack
            .push(__DirectiveLocation::InlineFragment);
    }
    fn exit_inline_fragment(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _inline_fragment: &'a Positioned<InlineFragment>,
    ) {
        self.location_stack.pop();
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/known_directives_tests.rs"]
mod tests;
