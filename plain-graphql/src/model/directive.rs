use crate::{Enum, Object, model::__InputValue, registry};
use std::collections::HashSet;
#[doc = " A Directive can be adjacent to many parts of the GraphQL language, a"]
#[doc = " __DirectiveLocation describes one such possible adjacencies."]
#[derive(Debug, Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(internal, name = "__DirectiveLocation")]
pub enum __DirectiveLocation {
    #[doc = " Location adjacent to a query operation."]
    Query,
    #[doc = " Location adjacent to a mutation operation."]
    Mutation,
    #[doc = " Location adjacent to a subscription operation."]
    Subscription,
    #[doc = " Location adjacent to a field."]
    Field,
    #[doc = " Location adjacent to a fragment definition."]
    FragmentDefinition,
    #[doc = " Location adjacent to a fragment spread."]
    FragmentSpread,
    #[doc = " Location adjacent to an inline fragment."]
    InlineFragment,
    #[doc = " Location adjacent to a variable definition."]
    VariableDefinition,
    #[doc = " Location adjacent to a schema definition."]
    Schema,
    #[doc = " Location adjacent to a scalar definition."]
    Scalar,
    #[doc = " Location adjacent to an object type definition."]
    Object,
    #[doc = " Location adjacent to a field definition."]
    FieldDefinition,
    #[doc = " Location adjacent to an argument definition."]
    ArgumentDefinition,
    #[doc = " Location adjacent to an interface definition."]
    Interface,
    #[doc = " Location adjacent to a union definition."]
    Union,
    #[doc = " Location adjacent to an enum definition."]
    Enum,
    #[doc = " Location adjacent to an enum value definition."]
    EnumValue,
    #[doc = " Location adjacent to an input object type definition."]
    InputObject,
    #[doc = " Location adjacent to an input object field definition."]
    InputFieldDefinition,
}
pub struct __Directive<'a> {
    pub registry: &'a registry::Registry,
    pub type_names: &'a HashSet<&'a str>,
    pub directive: &'a registry::MetaDirective,
}
#[doc = " A Directive provides a way to describe alternate runtime execution and type"]
#[doc = " validation behavior in a GraphQL document."]
#[doc = ""]
#[doc = " In some cases, you need to provide options to alter GraphQL's execution"]
#[doc = " behavior in ways field arguments will not suffice, such as conditionally"]
#[doc = " including or skipping a field. Directives provide this by describing"]
#[doc = " additional information to the executor."]
#[Object(internal, name = "__Directive")]
impl<'a> __Directive<'a> {
    #[inline]
    async fn name(&self) -> &str {
        &self.directive.name
    }
    #[inline]
    async fn description(&self) -> Option<&str> {
        self.directive.description.as_deref()
    }
    #[inline]
    async fn locations(&self) -> &Vec<__DirectiveLocation> {
        &self.directive.locations
    }
    async fn args(
        &self,
        #[graphql(default = false)] include_deprecated: bool,
    ) -> Vec<__InputValue<'a>> {
        self.directive
            .args
            .values()
            .filter(|input_value| include_deprecated || !input_value.deprecation.is_deprecated())
            .map(|input_value| __InputValue {
                registry: self.registry,
                type_names: self.type_names,
                input_value,
            })
            .collect()
    }
    #[inline]
    async fn is_repeatable(&self) -> bool {
        self.directive.is_repeatable
    }
}
