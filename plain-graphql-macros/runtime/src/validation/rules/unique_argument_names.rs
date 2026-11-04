use crate::value::Value;
use crate::{
    Name, Positioned,
    parser::types::{Directive, Field},
    validation::visitor::{Visitor, VisitorContext},
};
use std::collections::HashSet;
#[derive(Default)]
pub struct UniqueArgumentNames<'a> {
    names: HashSet<&'a str>,
}
impl<'a> Visitor<'a> for UniqueArgumentNames<'a> {
    fn enter_directive(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _directive: &'a Positioned<Directive>,
    ) {
        self.names.clear();
    }
    fn enter_argument(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        name: &'a Positioned<Name>,
        _value: &'a Positioned<Value>,
    ) {
        if !self.names.insert(name.node.as_str()) {
            ctx.report_error(
                vec![name.pos],
                format!("There can only be one argument named \"{}\"", name),
            )
        }
    }
    fn enter_field(&mut self, _ctx: &mut VisitorContext<'a>, _field: &'a Positioned<Field>) {
        self.names.clear();
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/unique_argument_names_tests.rs"]
mod tests;
