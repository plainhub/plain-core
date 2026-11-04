use crate::{
    Name, Pos, Positioned,
    parser::types::{ExecutableDocument, FragmentDefinition, FragmentSpread, OperationDefinition},
    validation::{
        utils::Scope,
        visitor::{Visitor, VisitorContext},
    },
};
use std::collections::{HashMap, HashSet};
#[derive(Default)]
pub struct NoUnusedFragments<'a> {
    spreads: HashMap<Scope<'a>, Vec<&'a str>>,
    defined_fragments: HashSet<(&'a str, Pos)>,
    current_scope: Option<Scope<'a>>,
}
impl<'a> NoUnusedFragments<'a> {
    fn find_reachable_fragments(&self, from: &Scope<'a>, result: &mut HashSet<&'a str>) {
        if let Scope::Fragment(name) = *from {
            if result.contains(name) {
                return;
            } else {
                result.insert(name);
            }
        }
        if let Some(spreads) = self.spreads.get(from) {
            for spread in spreads {
                self.find_reachable_fragments(&Scope::Fragment(spread), result)
            }
        }
    }
}
impl<'a> Visitor<'a> for NoUnusedFragments<'a> {
    fn exit_document(&mut self, ctx: &mut VisitorContext<'a>, doc: &'a ExecutableDocument) {
        let mut reachable = HashSet::new();
        for (name, _) in doc.operations.iter() {
            self.find_reachable_fragments(
                &Scope::Operation(name.map(Name::as_str)),
                &mut reachable,
            );
        }
        for (fragment_name, pos) in &self.defined_fragments {
            if !reachable.contains(fragment_name) {
                ctx.report_error(
                    vec![*pos],
                    format!(r#"Fragment "{}" is never used"#, fragment_name),
                );
            }
        }
    }
    fn enter_operation_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        name: Option<&'a Name>,
        _operation_definition: &'a Positioned<OperationDefinition>,
    ) {
        self.current_scope = Some(Scope::Operation(name.map(Name::as_str)));
    }
    fn enter_fragment_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        name: &'a Name,
        fragment_definition: &'a Positioned<FragmentDefinition>,
    ) {
        self.current_scope = Some(Scope::Fragment(name));
        self.defined_fragments
            .insert((name, fragment_definition.pos));
    }
    fn enter_fragment_spread(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        fragment_spread: &'a Positioned<FragmentSpread>,
    ) {
        if let Some(ref scope) = self.current_scope {
            self.spreads
                .entry(*scope)
                .or_default()
                .push(&fragment_spread.node.fragment_name.node);
        }
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/no_unused_fragments_tests.rs"]
mod tests;
