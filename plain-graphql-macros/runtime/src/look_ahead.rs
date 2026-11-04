use crate::{
    Context, Name, Positioned, SelectionField,
    parser::types::{Field, FragmentDefinition, Selection, SelectionSet},
};
use std::collections::HashMap;
pub struct Lookahead<'a> {
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    fields: Vec<&'a Field>,
    context: &'a Context<'a>,
}
impl<'a> Lookahead<'a> {
    pub(crate) fn new(
        fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
        field: &'a Field,
        context: &'a Context<'a>,
    ) -> Self {
        Self {
            fragments,
            fields: vec![field],
            context,
        }
    }
    #[must_use]
    pub fn field(&self, name: &str) -> Self {
        let mut fields = Vec::new();
        for field in &self.fields {
            filter(&mut fields, self.fragments, &field.selection_set.node, name)
        }
        Self {
            fragments: self.fragments,
            fields,
            context: self.context,
        }
    }
    #[inline]
    pub fn exists(&self) -> bool {
        !self.fields.is_empty()
    }
    pub fn selection_fields(&self) -> Vec<SelectionField<'a>> {
        self.fields
            .iter()
            .map(|field| SelectionField {
                fragments: self.fragments,
                field,
                context: self.context,
            })
            .collect()
    }
}
impl<'a> From<SelectionField<'a>> for Lookahead<'a> {
    fn from(selection_field: SelectionField<'a>) -> Self {
        Lookahead {
            fragments: selection_field.fragments,
            fields: vec![selection_field.field],
            context: selection_field.context,
        }
    }
}
impl<'a> TryFrom<&[SelectionField<'a>]> for Lookahead<'a> {
    type Error = ();
    fn try_from(selection_fields: &[SelectionField<'a>]) -> Result<Self, Self::Error> {
        if selection_fields.is_empty() {
            Err(())
        } else {
            Ok(Lookahead {
                fragments: selection_fields[0].fragments,
                fields: selection_fields
                    .iter()
                    .map(|selection_field| selection_field.field)
                    .collect(),
                context: selection_fields[0].context,
            })
        }
    }
}
fn filter<'a>(
    fields: &mut Vec<&'a Field>,
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    selection_set: &'a SelectionSet,
    name: &str,
) {
    for item in &selection_set.items {
        match &item.node {
            Selection::Field(field) => {
                if field.node.name.node == name {
                    fields.push(&field.node)
                }
            }
            Selection::InlineFragment(fragment) => {
                filter(fields, fragments, &fragment.node.selection_set.node, name)
            }
            Selection::FragmentSpread(spread) => {
                if let Some(fragment) = fragments.get(&spread.node.fragment_name.node) {
                    filter(fields, fragments, &fragment.node.selection_set.node, name)
                }
            }
        }
    }
}
#[cfg(test)]
#[path = "../../../tests/plain-graphql/unit/look_ahead_tests.rs"]
mod tests;
