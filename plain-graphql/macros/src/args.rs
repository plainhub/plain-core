use darling::{
    FromDeriveInput, FromField, FromMeta, FromVariant,
    ast::{Data, Fields},
    util::{Ignored, SpannedValue},
};
use crate::case::{ToLowerCamelCase, ToPascalCase, ToShoutySnakeCase, ToSnakeCase};
use syn::{Attribute, Expr, Generics, Ident, Lit, LitBool, Path, Type, Visibility};
#[derive(FromMeta, Clone)]
#[darling(default)]
pub struct CacheControl {
    public: bool,
    private: bool,
    pub no_cache: bool,
    pub max_age: usize,
}
impl Default for CacheControl {
    fn default() -> Self {
        Self {
            public: true,
            private: false,
            no_cache: false,
            max_age: 0,
        }
    }
}
impl CacheControl {
    pub fn is_public(&self) -> bool {
        !self.private && self.public
    }
}
#[derive(Debug)]
pub enum DefaultValue {
    Default,
    Value(Lit),
}
impl FromMeta for DefaultValue {
    fn from_word() -> darling::Result<Self> {
        Ok(DefaultValue::Default)
    }
    fn from_value(value: &Lit) -> darling::Result<Self> {
        Ok(DefaultValue::Value(value.clone()))
    }
}
#[derive(Debug, Clone, Default)]
pub enum Deprecation {
    #[default]
    NoDeprecated,
    Deprecated {
        reason: Option<String>,
    },
}
impl FromMeta for Deprecation {
    fn from_word() -> darling::Result<Self> {
        Ok(Deprecation::Deprecated { reason: None })
    }
    fn from_value(value: &Lit) -> darling::Result<Self> {
        match value {
            Lit::Bool(LitBool { value: true, .. }) => Ok(Deprecation::Deprecated { reason: None }),
            Lit::Bool(LitBool { value: false, .. }) => Ok(Deprecation::NoDeprecated),
            Lit::Str(str) => Ok(Deprecation::Deprecated {
                reason: Some(str.value()),
            }),
            _ => Err(darling::Error::unexpected_lit_type(value)),
        }
    }
}
#[derive(FromField)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct SimpleObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
    pub vis: Visibility,
    pub attrs: Vec<Attribute>,
    #[darling(default)]
    pub skip: bool,
    #[darling(default)]
    pub getter: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub deprecation: Deprecation,
    #[darling(default)]
    pub cache_control: CacheControl,
    #[darling(default)]
    pub guard: Option<Expr>,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct SimpleObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, SimpleObjectField>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub complex: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub cache_control: CacheControl,
    #[darling(default)]
    pub guard: Option<Expr>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Argument {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    pub deprecation: Deprecation,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Object {
    pub internal: bool,
    pub name: Option<String>,
    pub cache_control: CacheControl,
    #[darling(default)]
    pub guard: Option<Expr>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct ObjectField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Enum {
    pub ident: Ident,
    pub attrs: Vec<Attribute>,
    pub data: Data<EnumItem, Ignored>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub rename_items: Option<RenameRule>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromVariant)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct EnumItem {
    pub ident: Ident,
    pub attrs: Vec<Attribute>,
    pub fields: Fields<Ignored>,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub deprecation: Deprecation,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Union {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<UnionItem, Ignored>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromVariant)]
#[darling(attributes(graphql))]
pub struct UnionItem {
    pub ident: Ident,
    pub fields: Fields<syn::Type>,
}
#[derive(FromField)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct InputObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
    pub attrs: Vec<Attribute>,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub default: Option<DefaultValue>,
    #[darling(default)]
    pub skip: bool,
    #[darling(default)]
    pub deprecation: Deprecation,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct InputObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, InputObjectField>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta)]
pub struct InterfaceFieldArgument {
    pub name: String,
    #[darling(default)]
    pub desc: Option<String>,
    pub ty: Type,
    #[darling(default)]
    pub default: Option<DefaultValue>,
    #[darling(default)]
    pub deprecation: Deprecation,
}
#[derive(FromMeta)]
pub struct InterfaceField {
    pub name: SpannedValue<String>,
    pub ty: Type,
    #[darling(default)]
    pub method: Option<String>,
    #[darling(default)]
    pub desc: Option<String>,
    #[darling(default, multiple, rename = "arg")]
    pub args: Vec<InterfaceFieldArgument>,
    #[darling(default)]
    pub deprecation: Deprecation,
}
#[derive(FromVariant)]
pub struct InterfaceMember {
    pub ident: Ident,
    pub fields: Fields<syn::Type>,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Interface {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<InterfaceMember, Ignored>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default, multiple, rename = "field")]
    pub fields: Vec<InterfaceField>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Scalar {
    pub internal: bool,
    pub name: Option<String>,
    pub specified_by_url: Option<String>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Subscription {
    pub internal: bool,
    pub name: Option<String>,
    #[darling(default)]
    pub guard: Option<Expr>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct SubscriptionFieldArgument {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    pub deprecation: Deprecation,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct SubscriptionField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub guard: Option<Expr>,
}
#[derive(FromField)]
pub struct MergedObjectField {
    pub ty: Type,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct MergedObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, MergedObjectField>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromField)]
pub struct MergedSubscriptionField {
    pub ty: Type,
}
#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct MergedSubscription {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, MergedSubscriptionField>,
    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(Debug, Copy, Clone, FromMeta)]
pub enum RenameRule {
    #[darling(rename = "lowercase")]
    Lower,
    #[darling(rename = "UPPERCASE")]
    Upper,
    #[darling(rename = "PascalCase")]
    Pascal,
    #[darling(rename = "camelCase")]
    Camel,
    #[darling(rename = "snake_case")]
    Snake,
    #[darling(rename = "SCREAMING_SNAKE_CASE")]
    ScreamingSnake,
}
impl RenameRule {
    fn rename(&self, name: impl AsRef<str>) -> String {
        match self {
            Self::Lower => name.as_ref().to_lowercase(),
            Self::Upper => name.as_ref().to_uppercase(),
            Self::Pascal => name.as_ref().to_pascal_case(),
            Self::Camel => name.as_ref().to_lower_camel_case(),
            Self::Snake => name.as_ref().to_snake_case(),
            Self::ScreamingSnake => name.as_ref().to_shouty_snake_case(),
        }
    }
}
#[derive(Debug, Copy, Clone)]
pub enum RenameTarget {
    Type,
    EnumItem,
    Field,
    Argument,
}
impl RenameTarget {
    fn rule(&self) -> RenameRule {
        match self {
            RenameTarget::Type => RenameRule::Pascal,
            RenameTarget::EnumItem => RenameRule::ScreamingSnake,
            RenameTarget::Field => RenameRule::Camel,
            RenameTarget::Argument => RenameRule::Camel,
        }
    }
    pub fn rename(&self, name: impl AsRef<str>) -> String {
        self.rule().rename(name)
    }
}
pub trait RenameRuleExt {
    fn rename(&self, name: impl AsRef<str>, target: RenameTarget) -> String;
}
impl RenameRuleExt for Option<RenameRule> {
    fn rename(&self, name: impl AsRef<str>, target: RenameTarget) -> String {
        self.unwrap_or(target.rule()).rename(name)
    }
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct ComplexObject {
    pub internal: bool,
    pub guard: Option<Expr>,
    #[darling(rename = "crate")]
    pub crate_path: Option<Path>,
}
#[derive(FromMeta, Default)]
#[darling(default)]
pub struct ComplexObjectField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
}
