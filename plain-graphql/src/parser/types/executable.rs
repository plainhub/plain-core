use super::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableDocument {
    pub operations: DocumentOperations,
    pub fragments: HashMap<Name, Positioned<FragmentDefinition>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentOperations {
    Single(Positioned<OperationDefinition>),
    Multiple(HashMap<Name, Positioned<OperationDefinition>>),
}
impl DocumentOperations {
    #[must_use]
    pub fn iter(&self) -> OperationsIter<'_> {
        OperationsIter(match self {
            Self::Single(op) => OperationsIterInner::Single(Some(op)),
            Self::Multiple(ops) => OperationsIterInner::Multiple(ops.iter()),
        })
    }
}
#[derive(Debug, Clone)]
pub struct OperationsIter<'a>(OperationsIterInner<'a>);
impl<'a> Iterator for OperationsIter<'a> {
    type Item = (Option<&'a Name>, &'a Positioned<OperationDefinition>);
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            OperationsIterInner::Single(op) => op.take().map(|op| (None, op)),
            OperationsIterInner::Multiple(iter) => iter.next().map(|(name, op)| (Some(name), op)),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.len();
        (size, Some(size))
    }
}
impl std::iter::FusedIterator for OperationsIter<'_> {}
impl ExactSizeIterator for OperationsIter<'_> {
    fn len(&self) -> usize {
        match &self.0 {
            OperationsIterInner::Single(opt) => usize::from(opt.is_some()),
            OperationsIterInner::Multiple(iter) => iter.len(),
        }
    }
}
#[derive(Debug, Clone)]
enum OperationsIterInner<'a> {
    Single(Option<&'a Positioned<OperationDefinition>>),
    Multiple(hash_map::Iter<'a, Name, Positioned<OperationDefinition>>),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationDefinition {
    pub ty: OperationType,
    pub variable_definitions: Vec<Positioned<VariableDefinition>>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub name: Positioned<Name>,
    pub var_type: Positioned<Type>,
    pub directives: Vec<Positioned<Directive>>,
    pub default_value: Option<Positioned<ConstValue>>,
}
impl VariableDefinition {
    #[must_use]
    pub fn default_value(&self) -> Option<&ConstValue> {
        self.default_value.as_ref().map(|value| &value.node).or({
            if self.var_type.node.nullable {
                Some(&ConstValue::Null)
            } else {
                None
            }
        })
    }
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SelectionSet {
    pub items: Vec<Positioned<Selection>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Selection {
    Field(Positioned<Field>),
    FragmentSpread(Positioned<FragmentSpread>),
    InlineFragment(Positioned<InlineFragment>),
}
impl Selection {
    #[must_use]
    pub fn directives(&self) -> &Vec<Positioned<Directive>> {
        match self {
            Self::Field(field) => &field.node.directives,
            Self::FragmentSpread(spread) => &spread.node.directives,
            Self::InlineFragment(fragment) => &fragment.node.directives,
        }
    }
    #[must_use]
    pub fn directives_mut(&mut self) -> &mut Vec<Positioned<Directive>> {
        match self {
            Self::Field(field) => &mut field.node.directives,
            Self::FragmentSpread(spread) => &mut spread.node.directives,
            Self::InlineFragment(fragment) => &mut fragment.node.directives,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub alias: Option<Positioned<Name>>,
    pub name: Positioned<Name>,
    pub arguments: Vec<(Positioned<Name>, Positioned<Value>)>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}
impl Field {
    #[must_use]
    pub fn response_key(&self) -> &Positioned<Name> {
        self.alias.as_ref().unwrap_or(&self.name)
    }
    #[must_use]
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<Value>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentSpread {
    pub fragment_name: Positioned<Name>,
    pub directives: Vec<Positioned<Directive>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineFragment {
    pub type_condition: Option<Positioned<TypeCondition>>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentDefinition {
    pub type_condition: Positioned<TypeCondition>,
    pub directives: Vec<Positioned<Directive>>,
    pub selection_set: Positioned<SelectionSet>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCondition {
    pub on: Positioned<Name>,
}
