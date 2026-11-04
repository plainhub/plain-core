use crate::parser::types::Field;
use crate::{
    Positioned,
    validation::visitor::{VisitMode, Visitor, VisitorContext},
};
pub struct DepthCalculate<'a> {
    max_depth: &'a mut usize,
    current_depth: usize,
}
impl<'a> DepthCalculate<'a> {
    pub fn new(max_depth: &'a mut usize) -> Self {
        Self {
            max_depth,
            current_depth: 0,
        }
    }
}
impl<'ctx> Visitor<'ctx> for DepthCalculate<'_> {
    fn mode(&self) -> VisitMode {
        VisitMode::Inline
    }
    fn enter_field(&mut self, _ctx: &mut VisitorContext<'ctx>, _field: &'ctx Positioned<Field>) {
        self.current_depth += 1;
        *self.max_depth = (*self.max_depth).max(self.current_depth);
    }
    fn exit_field(&mut self, _ctx: &mut VisitorContext<'ctx>, _field: &'ctx Positioned<Field>) {
        self.current_depth -= 1;
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/visitors/depth_tests.rs"]
mod tests;
