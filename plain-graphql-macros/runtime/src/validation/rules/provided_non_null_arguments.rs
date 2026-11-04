use crate::{
    Positioned,
    parser::types::{Directive, Field},
    registry::MetaTypeName,
    validation::visitor::{Visitor, VisitorContext},
};
#[derive(Default)]
pub struct ProvidedNonNullArguments;
impl<'a> Visitor<'a> for ProvidedNonNullArguments {
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
            for arg in schema_directive.args.values() {
                if MetaTypeName::create(&arg.ty).is_non_null()
                    && arg.default_value.is_none()
                    && !directive
                        .node
                        .arguments
                        .iter()
                        .any(|(name, _)| name.node == arg.name)
                {
                    ctx . report_error (vec ! [directive . pos] , format ! ("Directive \"@{}\" argument \"{}\" of type \"{}\" is required but not provided" , directive . node . name , arg . name , arg . ty)) ;
                }
            }
        }
    }
    fn enter_field(&mut self, ctx: &mut VisitorContext<'a>, field: &'a Positioned<Field>) {
        if let Some(parent_type) = ctx.parent_type()
            && let Some(schema_field) = parent_type.field_by_name(&field.node.name.node)
        {
            for arg in schema_field.args.values() {
                if MetaTypeName::create(&arg.ty).is_non_null()
                    && arg.default_value.is_none()
                    && !field
                        .node
                        .arguments
                        .iter()
                        .any(|(name, _)| name.node == arg.name)
                {
                    ctx.report_error(
                        vec![field.pos],
                        format!(
                            r#"Field "{}" argument "{}" of type "{}" is required but not provided"#,
                            field.node.name,
                            arg.name,
                            parent_type.name()
                        ),
                    );
                }
            }
        }
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/provided_non_null_arguments_tests.rs"]
mod tests;
