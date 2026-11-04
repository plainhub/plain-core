use crate::{
    Positioned,
    parser::types::FragmentSpread,
    validation::visitor::{Visitor, VisitorContext},
};
#[derive(Default)]
pub struct KnownFragmentNames;
impl<'a> Visitor<'a> for KnownFragmentNames {
    fn enter_fragment_spread(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        fragment_spread: &'a Positioned<FragmentSpread>,
    ) {
        if !ctx.is_known_fragment(&fragment_spread.node.fragment_name.node) {
            ctx.report_error(
                vec![fragment_spread.pos],
                format!(
                    r#"Unknown fragment: "{}""#,
                    fragment_spread.node.fragment_name.node
                ),
            );
        }
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/known_fragment_names_tests.rs"]
mod tests;
