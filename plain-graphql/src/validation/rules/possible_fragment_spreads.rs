use crate::{
    Positioned,
    parser::types::{ExecutableDocument, FragmentSpread, InlineFragment, TypeCondition},
    validation::visitor::{Visitor, VisitorContext},
};
use std::collections::HashMap;
#[derive(Default)]
pub struct PossibleFragmentSpreads<'a> {
    fragment_types: HashMap<&'a str, &'a str>,
}
impl<'a> Visitor<'a> for PossibleFragmentSpreads<'a> {
    fn enter_document(&mut self, _ctx: &mut VisitorContext<'a>, doc: &'a ExecutableDocument) {
        for (name, fragment) in doc.fragments.iter() {
            self.fragment_types
                .insert(name.as_str(), &fragment.node.type_condition.node.on.node);
        }
    }
    fn enter_fragment_spread(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        fragment_spread: &'a Positioned<FragmentSpread>,
    ) {
        if let Some(fragment_type) = self
            .fragment_types
            .get(&*fragment_spread.node.fragment_name.node)
            && let Some(current_type) = ctx.current_type()
            && let Some(on_type) = ctx.registry.types.get(*fragment_type)
            && !current_type.type_overlap(on_type)
        {
            ctx . report_error (vec ! [fragment_spread . pos] , format ! ("Fragment \"{}\" cannot be spread here as objects of type \"{}\" can never be of type \"{}\"" , fragment_spread . node . fragment_name . node , current_type . name () , fragment_type) ,) ;
        }
    }
    fn enter_inline_fragment(
        &mut self,
        ctx: &mut VisitorContext<'a>,
        inline_fragment: &'a Positioned<InlineFragment>,
    ) {
        if let Some(parent_type) = ctx.parent_type()
            && let Some(TypeCondition { on: fragment_type }) = &inline_fragment
                .node
                .type_condition
                .as_ref()
                .map(|c| &c.node)
            && let Some(on_type) = ctx.registry.types.get(fragment_type.node.as_str())
            && !parent_type.type_overlap(&on_type)
        {
            ctx.report_error(
                vec![inline_fragment.pos],
                format!(
                    "Fragment cannot be spread here as objects of type \"{}\" \
             can never be of type \"{}\"",
                    parent_type.name(),
                    fragment_type
                ),
            )
        }
    }
}
#[cfg(test)]
#[path = "../../../../tests/plain-graphql/unit/validation/rules/possible_fragment_spreads_tests.rs"]
mod tests;
