use crate::Enum;
#[doc = " An enum describing what kind of type a given `__Type` is."]
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(internal, name = "__TypeKind")]
pub enum __TypeKind {
    #[doc = " Indicates this type is a scalar."]
    Scalar,
    #[doc = " Indicates this type is an object. `fields` and `interfaces` are valid"]
    #[doc = " fields."]
    Object,
    #[doc = " Indicates this type is an interface. `fields` and `possibleTypes` are"]
    #[doc = " valid fields."]
    Interface,
    #[doc = " Indicates this type is a union. `possibleTypes` is a valid field."]
    Union,
    #[doc = " Indicates this type is an enum. `enumValues` is a valid field."]
    Enum,
    #[doc = " Indicates this type is an input object. `inputFields` is a valid field."]
    InputObject,
    #[doc = " Indicates this type is a list. `ofType` is a valid field."]
    List,
    #[doc = " Indicates this type is a non-null. `ofType` is a valid field."]
    NonNull,
}
