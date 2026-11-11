use crate::value::Value;
use crate::{
    Name, Positioned, QueryPathSegment,
    context::QueryPathNode,
    parser::types::{Directive, Field},
    registry::MetaInputValue,
    validation::{
        utils::is_valid_input_value,
        visitor::{Visitor, VisitorContext},
    },
};
use plain_libs::indexmap::map::IndexMap;
#[derive(Default)]
pub struct ArgumentsOfCorrectType<'a> {
    current_args: Option<&'a IndexMap<String, MetaInputValue>>,
    in_unselected_operation: bool,
}
impl<'a> Visitor<'a> for ArgumentsOfCorrectType<'a> {
    fn enter_operation_definition(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        name: Option<&'a Name>,
        _operation_definition: &'a Positioned<crate::parser::types::OperationDefinition>,
    ) {
        self.in_unselected_operation =
            ctx.operation_name
                .zip(name)
                .is_some_and(|(selected_operation, current_operation)| {
                    selected_operation != current_operation.as_str()
                })
    }
    fn enter_directive(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        directive: &'a Positioned<Directive>,
    ) {
        self.current_args = ctx
            .registry
            .directives
            .get(directive.node.name.node.as_str())
            .map(|d| &d.args);
    }
    fn exit_directive(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _directive: &'a Positioned<Directive>,
    ) {
        self.current_args = None;
    }
    fn enter_argument(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        name: &'a Positioned<Name>,
        value: &'a Positioned<Value>,
    ) {
        if let Some(arg) = self
            .current_args
            .and_then(|args| args.get(name.node.as_str()))
        {
            let value = value
                .node
                .clone()
                .into_const_with(|var_name| {
                    (!self.in_unselected_operation)
                        .then_some(ctx.variables)
                        .flatten()
                        .and_then(|variables| variables.get(&var_name))
                        .cloned()
                        .ok_or(())
                })
                .ok();
            if let Some(reason) = value.and_then(|value| {
                is_valid_input_value(
                    ctx.registry,
                    &arg.ty,
                    &value,
                    QueryPathNode {
                        parent: None,
                        segment: QueryPathSegment::Name(&arg.name),
                    },
                )
            }) {
                ctx.report_error(
                    vec![name.pos],
                    format!("Invalid value for argument {}", reason),
                );
            }
        }
    }
    fn enter_field(&mut self, ctx: &mut VisitorContext<'a>, field: &'a Positioned<Field>) {
        self.current_args = ctx
            .parent_type()
            .and_then(|p| p.field_by_name(&field.node.name.node))
            .map(|f| &f.args);
    }
    fn exit_field(&mut self, _ctx: &mut VisitorContext<'a>, _field: &'a Positioned<Field>) {
        self.current_args = None;
    }
}
#[cfg(test)]
#[path = "../../../../tests/plain-graphql/unit/validation/rules/arguments_of_correct_type_tests.rs"]
mod tests;
