use crate::{
    Object,
    model::{__Directive, __Type},
    registry,
};
use std::collections::HashSet;
pub struct __Schema<'a> {
    registry: &'a registry::Registry,
    type_names: &'a HashSet<&'a str>,
}
impl<'a> __Schema<'a> {
    pub fn new(registry: &'a registry::Registry, type_names: &'a HashSet<&'a str>) -> Self {
        Self {
            registry,
            type_names,
        }
    }
}
#[doc = " A GraphQL Schema defines the capabilities of a GraphQL server. It exposes"]
#[doc = " all available types and directives on the server, as well as the entry"]
#[doc = " points for query, mutation, and subscription operations."]
#[Object(internal, name = "__Schema")]
impl<'a> __Schema<'a> {
    #[doc = " description of __Schema for newer graphiql introspection schema"]
    #[doc = " requirements"]
    async fn description(&self) -> String {
        String::from(
            "A GraphQL Schema defines the capabilities of a GraphQL server. It exposes all available types and directives on the server, as well as the entry points for query, mutation, and subscription operations.",
        )
    }
    #[doc = " A list of all types supported by this server."]
    async fn types(&self) -> Vec<__Type<'a>> {
        let mut types: Vec<_> = self
            .registry
            .types
            .values()
            .filter_map(|ty| {
                if self.type_names.contains(ty.name()) {
                    Some((
                        ty.name(),
                        __Type::new_simple(self.registry, self.type_names, ty),
                    ))
                } else {
                    None
                }
            })
            .collect();
        types.sort_by(|a, b| a.0.cmp(b.0));
        types.into_iter().map(|(_, ty)| ty).collect()
    }
    #[doc = " The type that query operations will be rooted at."]
    #[inline]
    async fn query_type(&self) -> __Type<'a> {
        __Type::new_simple(
            self.registry,
            self.type_names,
            &self.registry.types[&self.registry.query_type],
        )
    }
    #[doc = " If this server supports mutation, the type that mutation operations will"]
    #[doc = " be rooted at."]
    #[inline]
    async fn mutation_type(&self) -> Option<__Type<'a>> {
        self.registry.mutation_type.as_ref().and_then(|ty| {
            if self.type_names.contains(ty.as_str()) {
                Some(__Type::new_simple(
                    self.registry,
                    self.type_names,
                    &self.registry.types[ty],
                ))
            } else {
                None
            }
        })
    }
    #[doc = " If this server support subscription, the type that subscription"]
    #[doc = " operations will be rooted at."]
    #[inline]
    async fn subscription_type(&self) -> Option<__Type<'a>> {
        self.registry.subscription_type.as_ref().and_then(|ty| {
            if self.type_names.contains(ty.as_str()) {
                Some(__Type::new_simple(
                    self.registry,
                    self.type_names,
                    &self.registry.types[ty],
                ))
            } else {
                None
            }
        })
    }
    #[doc = " A list of all directives supported by this server."]
    async fn directives(&self) -> Vec<__Directive<'a>> {
        let mut directives: Vec<_> = self
            .registry
            .directives
            .values()
            .map(|directive| __Directive {
                registry: self.registry,
                type_names: self.type_names,
                directive,
            })
            .collect();
        directives.sort_by(|a, b| a.directive.name.cmp(&b.directive.name));
        directives
    }
}
