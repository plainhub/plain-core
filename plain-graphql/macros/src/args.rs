use crate::case::{ToLowerCamelCase, ToPascalCase, ToShoutySnakeCase, ToSnakeCase};
use crate::meta::{self, Data, Fields, FromMeta, Options};
use syn::{
    Attribute, DeriveInput, Expr, Field, Generics, Ident, Lit, LitBool, Meta, Path, Result, Type,
    Variant, Visibility,
};
#[derive(Clone)]
pub struct CacheControl {
    public: bool,
    private: bool,
    pub no_cache: bool,
    pub max_age: usize,
}
impl FromMeta for CacheControl {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            public: options.optional("public")?.unwrap_or(true),
            private: options.optional("private")?.unwrap_or_default(),
            no_cache: options.optional("no_cache")?.unwrap_or_default(),
            max_age: options.optional("max_age")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
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
    fn from_meta(value: &Meta) -> Result<Self> {
        if matches!(value, Meta::Path(_)) {
            Ok(Self::Default)
        } else {
            Ok(Self::Value(meta::literal(value)?.clone()))
        }
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
    fn from_meta(value: &Meta) -> Result<Self> {
        if matches!(value, Meta::Path(_)) {
            return Ok(Self::Deprecated { reason: None });
        }
        match meta::literal(value)? {
            Lit::Bool(LitBool { value: true, .. }) => Ok(Self::Deprecated { reason: None }),
            Lit::Bool(LitBool { value: false, .. }) => Ok(Self::NoDeprecated),
            Lit::Str(reason) => Ok(Self::Deprecated {
                reason: Some(reason.value()),
            }),
            lit => Err(syn::Error::new_spanned(
                lit,
                "Expected a boolean or deprecation reason",
            )),
        }
    }
}
pub struct SimpleObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
    pub vis: Visibility,
    pub attrs: Vec<Attribute>,
    pub skip: bool,
    pub getter: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
}
impl SimpleObjectField {
    pub fn from_field(input: &Field) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            ty: input.ty.clone(),
            vis: input.vis.clone(),
            attrs: meta::docs(&input.attrs),
            skip: options.optional("skip")?.unwrap_or_default(),
            getter: options.optional("getter")?.unwrap_or_default(),
            name: options.optional("name")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
            cache_control: options.optional("cache_control")?.unwrap_or_default(),
            guard: options.optional("guard")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct SimpleObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<(), SimpleObjectField>,
    pub internal: bool,
    pub complex: bool,
    pub name: Option<String>,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
    pub crate_path: Option<Path>,
}
impl SimpleObject {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, |_| Ok(()), SimpleObjectField::from_field)?,
            internal: options.optional("internal")?.unwrap_or_default(),
            complex: options.optional("complex")?.unwrap_or_default(),
            name: options.optional("name")?,
            cache_control: options.optional("cache_control")?.unwrap_or_default(),
            guard: options.optional("guard")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct Argument {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    pub deprecation: Deprecation,
}
impl FromMeta for Argument {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            name: options.optional("name")?,
            desc: options.optional("desc")?,
            default: options.optional("default")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct Object {
    pub internal: bool,
    pub name: Option<String>,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
    pub crate_path: Option<Path>,
}
impl FromMeta for Object {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            cache_control: options.optional("cache_control")?.unwrap_or_default(),
            guard: options.optional("guard")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct ObjectField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
}
impl FromMeta for ObjectField {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            skip: options.optional("skip")?.unwrap_or_default(),
            name: options.optional("name")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
            cache_control: options.optional("cache_control")?.unwrap_or_default(),
            guard: options.optional("guard")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct Enum {
    pub ident: Ident,
    pub attrs: Vec<Attribute>,
    pub data: Data<EnumItem, ()>,
    pub internal: bool,
    pub name: Option<String>,
    pub rename_items: Option<RenameRule>,
    pub crate_path: Option<Path>,
}
impl Enum {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, EnumItem::from_variant, |_| Ok(()))?,
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            rename_items: options.optional("rename_items")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct EnumItem {
    pub ident: Ident,
    pub attrs: Vec<Attribute>,
    pub fields: Fields<()>,
    pub name: Option<String>,
    pub deprecation: Deprecation,
}
impl EnumItem {
    pub fn from_variant(input: &Variant) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            attrs: meta::docs(&input.attrs),
            fields: Fields::parse(&input.fields, |_| Ok(()))?,
            name: options.optional("name")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct Union {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<UnionItem, ()>,
    pub internal: bool,
    pub name: Option<String>,
    pub crate_path: Option<Path>,
}
impl Union {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, UnionItem::from_variant, |_| Ok(()))?,
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct UnionItem {
    pub ident: Ident,
    pub fields: Fields<syn::Type>,
}
impl UnionItem {
    pub fn from_variant(input: &Variant) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            fields: Fields::parse(&input.fields, |field| Ok(field.ty.clone()))?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct InputObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
    pub attrs: Vec<Attribute>,
    pub name: Option<String>,
    pub default: Option<DefaultValue>,
    pub skip: bool,
    pub deprecation: Deprecation,
}
impl InputObjectField {
    pub fn from_field(input: &Field) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            ty: input.ty.clone(),
            attrs: meta::docs(&input.attrs),
            name: options.optional("name")?,
            default: options.optional("default")?,
            skip: options.optional("skip")?.unwrap_or_default(),
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct InputObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<(), InputObjectField>,
    pub internal: bool,
    pub name: Option<String>,
    pub crate_path: Option<Path>,
}
impl InputObject {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, |_| Ok(()), InputObjectField::from_field)?,
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct InterfaceFieldArgument {
    pub name: String,
    pub desc: Option<String>,
    pub ty: Type,
    pub default: Option<DefaultValue>,
    pub deprecation: Deprecation,
}
impl FromMeta for InterfaceFieldArgument {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            name: options.required("name")?,
            desc: options.optional("desc")?,
            ty: options.required("ty")?,
            default: options.optional("default")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct InterfaceField {
    pub name: String,
    pub ty: Type,
    pub method: Option<String>,
    pub desc: Option<String>,
    pub args: Vec<InterfaceFieldArgument>,
    pub deprecation: Deprecation,
}
impl FromMeta for InterfaceField {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            name: options.required("name")?,
            ty: options.required("ty")?,
            method: options.optional("method")?,
            desc: options.optional("desc")?,
            args: options.multiple("arg")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct InterfaceMember {
    pub ident: Ident,
    pub fields: Fields<syn::Type>,
}
impl InterfaceMember {
    pub fn from_variant(input: &Variant) -> Result<Self> {
        Ok(Self {
            ident: input.ident.clone(),
            fields: Fields::parse(&input.fields, |field| Ok(field.ty.clone()))?,
        })
    }
}

pub struct Interface {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<InterfaceMember, ()>,
    pub internal: bool,
    pub name: Option<String>,
    pub fields: Vec<InterfaceField>,
    pub crate_path: Option<Path>,
}
impl Interface {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, InterfaceMember::from_variant, |_| Ok(()))?,
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            fields: options.multiple("field")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct Scalar {
    pub internal: bool,
    pub name: Option<String>,
    pub specified_by_url: Option<String>,
    pub crate_path: Option<Path>,
}
impl FromMeta for Scalar {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            specified_by_url: options.optional("specified_by_url")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct Subscription {
    pub internal: bool,
    pub name: Option<String>,
    pub guard: Option<Expr>,
    pub crate_path: Option<Path>,
}
impl FromMeta for Subscription {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            guard: options.optional("guard")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct SubscriptionFieldArgument {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    pub deprecation: Deprecation,
}
impl FromMeta for SubscriptionFieldArgument {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            name: options.optional("name")?,
            desc: options.optional("desc")?,
            default: options.optional("default")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct SubscriptionField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub guard: Option<Expr>,
}
impl FromMeta for SubscriptionField {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            skip: options.optional("skip")?.unwrap_or_default(),
            name: options.optional("name")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
            guard: options.optional("guard")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct MergedObjectField {
    pub ty: Type,
}
impl MergedObjectField {
    pub fn from_field(input: &Field) -> Result<Self> {
        Ok(Self {
            ty: input.ty.clone(),
        })
    }
}

pub struct MergedObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<(), MergedObjectField>,
    pub internal: bool,
    pub name: Option<String>,
    pub crate_path: Option<Path>,
}
impl MergedObject {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, |_| Ok(()), MergedObjectField::from_field)?,
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

pub struct MergedSubscriptionField {
    pub ty: Type,
}
impl MergedSubscriptionField {
    pub fn from_field(input: &Field) -> Result<Self> {
        Ok(Self {
            ty: input.ty.clone(),
        })
    }
}

pub struct MergedSubscription {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<(), MergedSubscriptionField>,
    pub internal: bool,
    pub name: Option<String>,
    pub crate_path: Option<Path>,
}
impl MergedSubscription {
    pub fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        let mut options = Options::from_attrs(&input.attrs)?;
        let value = Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            attrs: meta::docs(&input.attrs),
            data: Data::parse(input, |_| Ok(()), MergedSubscriptionField::from_field)?,
            internal: options.optional("internal")?.unwrap_or_default(),
            name: options.optional("name")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RenameRule {
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
}
impl FromMeta for RenameRule {
    fn from_meta(meta: &Meta) -> Result<Self> {
        if let Meta::List(list) = meta {
            let items = meta::parse_list(list.tokens.clone())?;
            if let [Meta::Path(path)] = items.as_slice()
                && let Some(name) = path.get_ident()
            {
                return Self::from_name(&name.to_string(), name.span());
            }
            return Err(syn::Error::new_spanned(
                list,
                "Expected one GraphQL rename rule",
            ));
        }
        let value = meta::string(meta)?;
        Self::from_name(&value.value(), value.span())
    }
}
impl RenameRule {
    fn from_name(name: &str, span: proc_macro2::Span) -> Result<Self> {
        match name {
            "lowercase" => Ok(Self::Lower),
            "UPPERCASE" => Ok(Self::Upper),
            "PascalCase" => Ok(Self::Pascal),
            "camelCase" => Ok(Self::Camel),
            "snake_case" => Ok(Self::Snake),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
            _ => Err(syn::Error::new(span, "Unknown GraphQL rename rule")),
        }
    }
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
#[derive(Default)]
pub struct ComplexObject {
    pub internal: bool,
    pub guard: Option<Expr>,
    pub crate_path: Option<Path>,
}
impl FromMeta for ComplexObject {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            internal: options.optional("internal")?.unwrap_or_default(),
            guard: options.optional("guard")?,
            crate_path: options.optional("crate")?,
        };
        options.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct ComplexObjectField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Deprecation,
    pub cache_control: CacheControl,
    pub guard: Option<Expr>,
}
impl FromMeta for ComplexObjectField {
    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let mut options = Options::new(items);
        let value = Self {
            skip: options.optional("skip")?.unwrap_or_default(),
            name: options.optional("name")?,
            deprecation: options.optional("deprecation")?.unwrap_or_default(),
            cache_control: options.optional("cache_control")?.unwrap_or_default(),
            guard: options.optional("guard")?,
        };
        options.finish()?;
        Ok(value)
    }
}
