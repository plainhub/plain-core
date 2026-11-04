use crate::parser::types::ExecutableDocument;
use crate::{
    Positioned,
    parser::types::Field,
    validation::visitor::{VisitMode, Visitor, VisitorContext},
};
pub struct ComplexityCalculate<'a> {
    pub complexity: &'a mut usize,
    pub complexity_stack: Vec<usize>,
}
impl<'a> ComplexityCalculate<'a> {
    pub fn new(complexity: &'a mut usize) -> Self {
        Self {
            complexity,
            complexity_stack: Default::default(),
        }
    }
}
impl<'ctx> Visitor<'ctx> for ComplexityCalculate<'_> {
    fn mode(&self) -> VisitMode {
        VisitMode::Inline
    }
    fn enter_document(&mut self, _ctx: &mut VisitorContext<'ctx>, _doc: &'ctx ExecutableDocument) {
        self.complexity_stack.push(0);
    }
    fn exit_document(&mut self, _ctx: &mut VisitorContext<'ctx>, _doc: &'ctx ExecutableDocument) {
        *self.complexity = self.complexity_stack.pop().unwrap();
    }

    fn enter_field(&mut self, _ctx: &mut VisitorContext<'_>, _field: &Positioned<Field>) {
        self.complexity_stack.push(0);
    }
    fn exit_field(&mut self, _ctx: &mut VisitorContext<'ctx>, _field: &'ctx Positioned<Field>) {
        let children = self.complexity_stack.pop().unwrap();
        *self.complexity_stack.last_mut().unwrap() += 1 + children;
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/visitors/complexity_tests.rs"]
mod tests;
