mod cache_control;
mod export_sdl;
pub use crate::model::__DirectiveLocation;
use crate::{
    ID, InputType, OutputType, SubscriptionType, Value,
    model::__Schema,
    parser::types::{BaseType as ParsedBaseType, Type as ParsedType},
    schema::IntrospectionMode,
};
pub use cache_control::CacheControl;

use indexmap::{map::IndexMap, set::IndexSet};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{self, Display, Formatter, Write},
    sync::Arc,
};
fn strip_brackets(type_name: &str) -> Option<&str> {
    type_name
        .strip_prefix('[')
        .map(|rest| &rest[..rest.len() - 1])
}
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum MetaTypeName<'a> {
    List(&'a str),
    NonNull(&'a str),
    Named(&'a str),
}
impl Display for MetaTypeName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MetaTypeName::Named(name) => write!(f, "{}", name),
            MetaTypeName::NonNull(name) => write!(f, "{}!", name),
            MetaTypeName::List(name) => write!(f, "[{}]", name),
        }
    }
}
impl MetaTypeName<'_> {
    #[inline]
    pub fn create(type_name: &str) -> MetaTypeName<'_> {
        if let Some(type_name) = type_name.strip_suffix('!') {
            MetaTypeName::NonNull(type_name)
        } else if let Some(type_name) = strip_brackets(type_name) {
            MetaTypeName::List(type_name)
        } else {
            MetaTypeName::Named(type_name)
        }
    }
    #[inline]
    pub fn concrete_typename(type_name: &str) -> &str {
        match MetaTypeName::create(type_name) {
            MetaTypeName::List(type_name) => Self::concrete_typename(type_name),
            MetaTypeName::NonNull(type_name) => Self::concrete_typename(type_name),
            MetaTypeName::Named(type_name) => type_name,
        }
    }
    #[inline]
    pub fn is_non_null(&self) -> bool {
        matches!(self, MetaTypeName::NonNull(_))
    }
    #[inline]
    #[must_use]
    pub fn unwrap_non_null(&self) -> Self {
        match self {
            MetaTypeName::NonNull(ty) => MetaTypeName::create(ty),
            _ => *self,
        }
    }
    #[inline]
    pub fn is_subtype(&self, sub: &MetaTypeName<'_>) -> bool {
        match (self, sub) {
            (MetaTypeName::NonNull(super_type), MetaTypeName::NonNull(sub_type))
            | (MetaTypeName::Named(super_type), MetaTypeName::NonNull(sub_type)) => {
                MetaTypeName::create(super_type).is_subtype(&MetaTypeName::create(sub_type))
            }
            (MetaTypeName::Named(super_type), MetaTypeName::Named(sub_type)) => {
                super_type == sub_type
            }
            (MetaTypeName::List(super_type), MetaTypeName::List(sub_type)) => {
                MetaTypeName::create(super_type).is_subtype(&MetaTypeName::create(sub_type))
            }
            _ => false,
        }
    }
    #[inline]
    pub fn is_list(&self) -> bool {
        match self {
            MetaTypeName::List(_) => true,
            MetaTypeName::NonNull(ty) => MetaTypeName::create(ty).is_list(),
            MetaTypeName::Named(name) => name.ends_with(']'),
        }
    }
}
#[derive(Clone)]
pub struct MetaInputValue {
    pub name: String,
    pub description: Option<String>,
    pub ty: String,
    pub deprecation: Deprecation,
    pub default_value: Option<String>,
}
impl MetaInputValue {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            ty: ty.into(),
            deprecation: Deprecation::NoDeprecated,
            default_value: None,
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
impl Deprecation {
    #[inline]
    pub fn is_deprecated(&self) -> bool {
        matches!(self, Deprecation::Deprecated { .. })
    }
    #[inline]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Deprecation::NoDeprecated => None,
            Deprecation::Deprecated { reason } => reason.as_deref(),
        }
    }
}
#[derive(Clone)]
pub struct MetaField {
    pub name: String,
    pub description: Option<String>,
    pub args: IndexMap<String, MetaInputValue>,
    pub ty: String,
    pub deprecation: Deprecation,
    pub cache_control: CacheControl,
}
impl MetaField {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            args: IndexMap::new(),
            ty: ty.into(),
            deprecation: Deprecation::NoDeprecated,
            cache_control: Default::default(),
        }
    }
}
#[derive(Clone)]
pub struct MetaEnumValue {
    pub name: String,
    pub description: Option<String>,
    pub deprecation: Deprecation,
}
impl MetaEnumValue {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            deprecation: Deprecation::NoDeprecated,
        }
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MetaTypeId {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
}
impl MetaTypeId {
    fn create_fake_type(&self, rust_typename: &'static str) -> MetaType {
        match self {
            MetaTypeId::Scalar => MetaType::Scalar {
                name: "".to_string(),
                description: None,
                is_valid: None,
                specified_by_url: None,
            },
            MetaTypeId::Object => MetaType::Object {
                name: "".to_string(),
                description: None,
                fields: Default::default(),
                cache_control: Default::default(),
                is_subscription: false,
                rust_typename: Some(rust_typename),
            },
            MetaTypeId::Interface => MetaType::Interface {
                name: "".to_string(),
                description: None,
                fields: Default::default(),
                possible_types: Default::default(),
                rust_typename: Some(rust_typename),
            },
            MetaTypeId::Union => MetaType::Union {
                name: "".to_string(),
                description: None,
                possible_types: Default::default(),
                rust_typename: Some(rust_typename),
            },
            MetaTypeId::Enum => MetaType::Enum {
                name: "".to_string(),
                description: None,
                enum_values: Default::default(),
                rust_typename: Some(rust_typename),
            },
            MetaTypeId::InputObject => MetaType::InputObject {
                name: "".to_string(),
                description: None,
                input_fields: Default::default(),
                rust_typename: Some(rust_typename),
                oneof: false,
            },
        }
    }
}
impl Display for MetaTypeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MetaTypeId::Scalar => "Scalar",
            MetaTypeId::Object => "Object",
            MetaTypeId::Interface => "Interface",
            MetaTypeId::Union => "Union",
            MetaTypeId::Enum => "Enum",
            MetaTypeId::InputObject => "InputObject",
        })
    }
}
pub type ScalarValidatorFn = Arc<dyn Fn(&Value) -> bool + Send + Sync>;
#[derive(Clone)]
pub enum MetaType {
    Scalar {
        name: String,
        description: Option<String>,
        is_valid: Option<ScalarValidatorFn>,
        specified_by_url: Option<String>,
    },
    Object {
        name: String,
        description: Option<String>,
        fields: IndexMap<String, MetaField>,
        cache_control: CacheControl,
        is_subscription: bool,
        rust_typename: Option<&'static str>,
    },
    Interface {
        name: String,
        description: Option<String>,
        fields: IndexMap<String, MetaField>,
        possible_types: IndexSet<String>,
        rust_typename: Option<&'static str>,
    },
    Union {
        name: String,
        description: Option<String>,
        possible_types: IndexSet<String>,
        rust_typename: Option<&'static str>,
    },
    Enum {
        name: String,
        description: Option<String>,
        enum_values: IndexMap<String, MetaEnumValue>,
        rust_typename: Option<&'static str>,
    },
    InputObject {
        name: String,
        description: Option<String>,
        input_fields: IndexMap<String, MetaInputValue>,
        rust_typename: Option<&'static str>,
        oneof: bool,
    },
}
#[derive(Clone)]
pub struct ObjectBuilder {
    name: String,
    description: Option<String>,
    fields: IndexMap<String, MetaField>,
    cache_control: CacheControl,
    is_subscription: bool,
    rust_typename: Option<&'static str>,
}
impl ObjectBuilder {
    pub fn new(name: impl Into<String>, fields: IndexMap<String, MetaField>) -> Self {
        Self {
            name: name.into(),
            description: None,
            fields,
            cache_control: Default::default(),
            is_subscription: false,
            rust_typename: None,
        }
    }
    #[must_use]
    pub fn description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }
    #[must_use]
    pub fn cache_control(mut self, cache_control: CacheControl) -> Self {
        self.cache_control = cache_control;
        self
    }

    #[must_use]
    pub fn is_subscription(mut self, is_subscription: bool) -> Self {
        self.is_subscription = is_subscription;
        self
    }
    #[must_use]
    pub fn rust_typename(mut self, rust_typename: &'static str) -> Self {
        self.rust_typename = Some(rust_typename);
        self
    }
    pub fn build(self) -> MetaType {
        MetaType::Object {
            name: self.name,
            description: self.description,
            fields: self.fields,
            cache_control: self.cache_control,
            is_subscription: self.is_subscription,
            rust_typename: self.rust_typename,
        }
    }
}
#[derive(Clone)]
pub struct EnumBuilder {
    name: String,
    description: Option<String>,
    enum_values: IndexMap<String, MetaEnumValue>,
    rust_typename: Option<&'static str>,
}
impl EnumBuilder {
    pub fn new(name: impl Into<String>, enum_values: IndexMap<String, MetaEnumValue>) -> Self {
        Self {
            name: name.into(),
            description: None,
            enum_values,
            rust_typename: None,
        }
    }
    #[must_use]
    pub fn description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    #[must_use]
    pub fn rust_typename(mut self, rust_typename: &'static str) -> Self {
        self.rust_typename = Some(rust_typename);
        self
    }
    pub fn build(self) -> MetaType {
        MetaType::Enum {
            name: self.name,
            description: self.description,
            enum_values: self.enum_values,
            rust_typename: self.rust_typename,
        }
    }
}
#[derive(Clone)]
pub struct InputObjectBuilder {
    name: String,
    description: Option<String>,
    input_fields: IndexMap<String, MetaInputValue>,
    rust_typename: Option<&'static str>,
    oneof: bool,
}
impl InputObjectBuilder {
    pub fn new(name: impl Into<String>, input_fields: IndexMap<String, MetaInputValue>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_fields,
            rust_typename: None,
            oneof: false,
        }
    }
    #[must_use]
    pub fn description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    #[must_use]
    pub fn rust_typename(mut self, rust_typename: &'static str) -> Self {
        self.rust_typename = Some(rust_typename);
        self
    }
    #[must_use]
    pub fn oneof(mut self, oneof: bool) -> Self {
        self.oneof = oneof;
        self
    }
    pub fn build(self) -> MetaType {
        MetaType::InputObject {
            name: self.name,
            description: self.description,
            input_fields: self.input_fields,
            rust_typename: self.rust_typename,
            oneof: self.oneof,
        }
    }
}
impl MetaType {
    #[inline]
    pub fn type_id(&self) -> MetaTypeId {
        match self {
            MetaType::Scalar { .. } => MetaTypeId::Scalar,
            MetaType::Object { .. } => MetaTypeId::Object,
            MetaType::Interface { .. } => MetaTypeId::Interface,
            MetaType::Union { .. } => MetaTypeId::Union,
            MetaType::Enum { .. } => MetaTypeId::Enum,
            MetaType::InputObject { .. } => MetaTypeId::InputObject,
        }
    }
    #[inline]
    pub fn field_by_name(&self, name: &str) -> Option<&MetaField> {
        self.fields().and_then(|fields| fields.get(name))
    }
    #[inline]
    pub fn fields(&self) -> Option<&IndexMap<String, MetaField>> {
        match self {
            MetaType::Object { fields, .. } => Some(&fields),
            MetaType::Interface { fields, .. } => Some(&fields),
            _ => None,
        }
    }

    #[inline]
    pub fn name(&self) -> &str {
        match self {
            MetaType::Scalar { name, .. } => &name,
            MetaType::Object { name, .. } => name,
            MetaType::Interface { name, .. } => name,
            MetaType::Union { name, .. } => name,
            MetaType::Enum { name, .. } => name,
            MetaType::InputObject { name, .. } => name,
        }
    }
    #[inline]
    pub fn is_composite(&self) -> bool {
        matches!(
            self,
            MetaType::Object { .. } | MetaType::Interface { .. } | MetaType::Union { .. }
        )
    }
    #[inline]
    pub fn is_abstract(&self) -> bool {
        matches!(self, MetaType::Interface { .. } | MetaType::Union { .. })
    }
    #[inline]
    pub fn is_leaf(&self) -> bool {
        matches!(self, MetaType::Enum { .. } | MetaType::Scalar { .. })
    }
    #[inline]
    pub fn is_input(&self) -> bool {
        matches!(
            self,
            MetaType::Enum { .. } | MetaType::Scalar { .. } | MetaType::InputObject { .. }
        )
    }
    #[inline]
    pub fn is_possible_type(&self, type_name: &str) -> bool {
        match self {
            MetaType::Interface { possible_types, .. } => possible_types.contains(type_name),
            MetaType::Union { possible_types, .. } => possible_types.contains(type_name),
            MetaType::Object { name, .. } => name == type_name,
            _ => false,
        }
    }
    #[inline]
    pub fn possible_types(&self) -> Option<&IndexSet<String>> {
        match self {
            MetaType::Interface { possible_types, .. } => Some(possible_types),
            MetaType::Union { possible_types, .. } => Some(possible_types),
            _ => None,
        }
    }
    pub fn type_overlap(&self, ty: &MetaType) -> bool {
        if std::ptr::eq(self, ty) {
            return true;
        }
        match (self.is_abstract(), ty.is_abstract()) {
            (true, true) => self
                .possible_types()
                .iter()
                .copied()
                .flatten()
                .any(|type_name| ty.is_possible_type(type_name)),
            (true, false) => self.is_possible_type(ty.name()),
            (false, true) => ty.is_possible_type(self.name()),
            (false, false) => false,
        }
    }
    pub fn rust_typename(&self) -> Option<&'static str> {
        match self {
            MetaType::Scalar { .. } => None,
            MetaType::Object { rust_typename, .. } => *rust_typename,
            MetaType::Interface { rust_typename, .. } => *rust_typename,
            MetaType::Union { rust_typename, .. } => *rust_typename,
            MetaType::Enum { rust_typename, .. } => *rust_typename,
            MetaType::InputObject { rust_typename, .. } => *rust_typename,
        }
    }
}
pub struct MetaDirective {
    pub name: String,
    pub description: Option<String>,
    pub locations: Vec<__DirectiveLocation>,
    pub args: IndexMap<String, MetaInputValue>,
    pub is_repeatable: bool,
}
impl MetaDirective {
    pub(crate) fn sdl(&self) -> String {
        let mut sdl = String::new();
        if let Some(description) = &self.description {
            self::export_sdl::write_description(&mut sdl, 0, description);
        }
        write!(sdl, "directive @{}", self.name).ok();
        if !self.args.is_empty() {
            let args = self
                .args
                .values()
                .map(|value| self.argument_sdl(value))
                .collect::<Vec<_>>()
                .join(", ");
            write!(sdl, "({})", args).ok();
        }
        let locations = self
            .locations
            .iter()
            .map(|location| location.to_value().to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        if self.is_repeatable {
            write!(sdl, " repeatable").ok();
        }
        write!(sdl, " on {}", locations).ok();
        sdl
    }
    pub(crate) fn argument_sdl(&self, argument: &MetaInputValue) -> String {
        let argument_default = match &argument.default_value {
            Some(default) => format!(" = {default}"),
            None => "".to_string(),
        };
        format!("{}: {}{}", argument.name, argument.ty, argument_default)
    }
}
#[derive(Default)]
pub struct Registry {
    pub types: BTreeMap<String, MetaType>,
    pub directives: BTreeMap<String, MetaDirective>,
    pub implements: HashMap<String, IndexSet<String>>,
    pub query_type: String,
    pub mutation_type: Option<String>,
    pub subscription_type: Option<String>,
    pub introspection_mode: IntrospectionMode,

    pub enable_suggestions: bool,
}
impl Registry {
    pub(crate) fn add_system_types(&mut self) {
        self . add_directive (MetaDirective { name : "skip" . into () , description : Some ("Directs the executor to skip this field or fragment when the `if` argument is true." . to_string ()) , locations : vec ! [__DirectiveLocation :: Field , __DirectiveLocation :: FragmentSpread , __DirectiveLocation :: InlineFragment] , args : { let mut args = IndexMap :: new () ; args . insert ("if" . to_string () , MetaInputValue { name : "if" . to_string () , description : Some ("Skipped when true." . to_string ()) , ty : "Boolean!" . to_string () , deprecation : Deprecation :: NoDeprecated , default_value : None ,  }) ; args } , is_repeatable : false , }) ;
        self . add_directive (MetaDirective { name : "include" . into () , description : Some ("Directs the executor to include this field or fragment only when the `if` argument is true." . to_string ()) , locations : vec ! [__DirectiveLocation :: Field , __DirectiveLocation :: FragmentSpread , __DirectiveLocation :: InlineFragment] , args : { let mut args = IndexMap :: new () ; args . insert ("if" . to_string () , MetaInputValue { name : "if" . to_string () , description : Some ("Included when true." . to_string ()) , ty : "Boolean!" . to_string () , deprecation : Deprecation :: NoDeprecated , default_value : None ,  }) ; args } , is_repeatable : false , }) ;
        self.add_directive(MetaDirective {
            name: "deprecated".into(),
            description: Some(
                "Marks an element of a GraphQL schema as no longer supported.".into(),
            ),
            locations: vec![
                __DirectiveLocation::FieldDefinition,
                __DirectiveLocation::ArgumentDefinition,
                __DirectiveLocation::InputFieldDefinition,
                __DirectiveLocation::EnumValue,
            ],
            args: {
                let mut args = IndexMap::new();
                args.insert(
                    "reason".into(),
                    MetaInputValue {
                        name: "reason".into(),
                        description: Some(
                            "A reason for why it is deprecated, formatted using Markdown syntax"
                                .into(),
                        ),
                        ty: "String".into(),
                        deprecation: Deprecation::NoDeprecated,
                        default_value: Some(r#""No longer supported""#.into()),
                    },
                );
                args
            },
            is_repeatable: false,
        });
        self . add_directive (MetaDirective { name : "specifiedBy" . into () , description : Some ("Provides a scalar specification URL for specifying the behavior of custom scalar types." . into ()) , locations : vec ! [__DirectiveLocation :: Scalar] , args : { let mut args = IndexMap :: new () ; args . insert ("url" . into () , MetaInputValue { name : "url" . into () , description : Some ("URL that specifies the behavior of this scalar." . into ()) , ty : "String!" . into () , deprecation : Deprecation :: NoDeprecated , default_value : None ,  } ,) ; args } , is_repeatable : false , }) ;
        self.add_directive(MetaDirective {
            name: "oneOf".into(),
            description: Some(
                "Indicates that an Input Object is a OneOf Input Object (and thus requires \
                exactly one of its field be provided)"
                    .to_string(),
            ),
            locations: vec![__DirectiveLocation::InputObject],
            args: Default::default(),
            is_repeatable: false,
        });
        <bool as InputType>::create_type_info(self);
        <i32 as InputType>::create_type_info(self);
        <f32 as InputType>::create_type_info(self);
        <String as InputType>::create_type_info(self);
        <ID as InputType>::create_type_info(self);
    }
    pub fn create_input_type<T, F>(&mut self, type_id: MetaTypeId, mut f: F) -> String
    where
        T: InputType,
        F: FnMut(&mut Registry) -> MetaType,
    {
        self.create_type(&mut f, &T::type_name(), std::any::type_name::<T>(), type_id);
        T::qualified_type_name()
    }
    pub fn create_output_type<T, F>(&mut self, type_id: MetaTypeId, mut f: F) -> String
    where
        T: OutputType + ?Sized,
        F: FnMut(&mut Registry) -> MetaType,
    {
        self.create_type(&mut f, &T::type_name(), std::any::type_name::<T>(), type_id);
        T::qualified_type_name()
    }
    pub fn create_subscription_type<T, F>(&mut self, mut f: F) -> String
    where
        T: SubscriptionType + ?Sized,
        F: FnMut(&mut Registry) -> MetaType,
    {
        self.create_type(
            &mut f,
            &T::type_name(),
            std::any::type_name::<T>(),
            MetaTypeId::Object,
        );
        T::qualified_type_name()
    }
    fn create_type<F: FnMut(&mut Registry) -> MetaType>(
        &mut self,
        f: &mut F,
        name: &str,
        rust_typename: &'static str,
        type_id: MetaTypeId,
    ) {
        match self.types.get(name) {
            Some(ty) => {
                if let Some(prev_typename) = ty.rust_typename() {
                    if prev_typename == "__fake_type__" {
                        return;
                    }
                    if rust_typename != prev_typename {
                        panic!(
                            "`{}` and `{}` have the same GraphQL name `{}`",
                            prev_typename, rust_typename, name,
                        );
                    }
                    if ty.type_id() != type_id {
                        panic!(
                            "Register `{}` as `{}`, but it is already registered as `{}`",
                            name,
                            type_id,
                            ty.type_id()
                        );
                    }
                }
            }
            None => {
                self.types
                    .insert(name.to_string(), type_id.create_fake_type(rust_typename));
                let ty = f(self);
                *self.types.get_mut(name).unwrap() = ty;
            }
        }
    }
    pub fn create_fake_output_type<T: OutputType>(&mut self) -> MetaType {
        T::create_type_info(self);
        self.types
            .get(&*T::type_name())
            .cloned()
            .expect("You definitely encountered a bug!")
    }
    pub fn create_fake_input_type<T: InputType>(&mut self) -> MetaType {
        T::create_type_info(self);
        self.types
            .get(&*T::type_name())
            .cloned()
            .expect("You definitely encountered a bug!")
    }
    pub fn create_fake_subscription_type<T: SubscriptionType>(&mut self) -> MetaType {
        T::create_type_info(self);
        self.types
            .get(&*T::type_name())
            .cloned()
            .expect("You definitely encountered a bug!")
    }
    pub fn add_directive(&mut self, directive: MetaDirective) {
        self.directives
            .insert(directive.name.to_string(), directive);
    }
    pub fn add_implements(&mut self, ty: &str, interface: &str) {
        self.implements
            .entry(ty.to_string())
            .and_modify(|interfaces| {
                interfaces.insert(interface.to_string());
            })
            .or_insert({
                let mut interfaces = IndexSet::new();
                interfaces.insert(interface.to_string());
                interfaces
            });
    }
    pub fn concrete_type_by_name(&self, type_name: &str) -> Option<&MetaType> {
        self.types.get(MetaTypeName::concrete_typename(type_name))
    }
    pub fn concrete_type_by_parsed_type(&self, query_type: &ParsedType) -> Option<&MetaType> {
        match &query_type.base {
            ParsedBaseType::Named(name) => self.types.get(name.as_str()),
            ParsedBaseType::List(ty) => self.concrete_type_by_parsed_type(ty),
        }
    }
    pub(crate) fn create_introspection_types(&mut self) {
        __Schema::create_type_info(self);
        if let Some(MetaType::Object { fields, .. }) = self.types.get_mut(&self.query_type) {
            fields.insert(
                "__schema".to_string(),
                MetaField {
                    name: "__schema".to_string(),
                    description: Some("Access the current type schema of this server.".to_string()),
                    args: Default::default(),
                    ty: "__Schema".to_string(),
                    deprecation: Default::default(),
                    cache_control: Default::default(),
                },
            );
            fields.insert(
                "__type".to_string(),
                MetaField {
                    name: "__type".to_string(),
                    description: Some("Request the type information of a single type.".to_string()),
                    args: {
                        let mut args = IndexMap::new();
                        args.insert(
                            "name".to_string(),
                            MetaInputValue {
                                name: "name".to_string(),
                                description: None,
                                ty: "String!".to_string(),
                                deprecation: Deprecation::NoDeprecated,
                                default_value: None,
                            },
                        );
                        args
                    },
                    ty: "__Type".to_string(),
                    deprecation: Default::default(),
                    cache_control: Default::default(),
                },
            );
        }
    }

    pub fn remove_unused_types(&mut self) {
        let used = self.referenced_types();
        let unused = self
            .types
            .keys()
            .filter(|name| !is_system_type(name) && !used.contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for name in unused {
            self.types.remove(&name);
        }
    }
    fn collect_type<'a>(&'a self, name: &'a str, used: &mut HashSet<&'a str>) {
        let Some(ty) = self.types.get(name) else {
            return;
        };
        if !used.insert(name) {
            return;
        }
        if let MetaType::Object { fields, .. } | MetaType::Interface { fields, .. } = ty {
            for field in fields.values() {
                self.collect_type(MetaTypeName::concrete_typename(&field.ty), used);
                for arg in field.args.values() {
                    self.collect_type(MetaTypeName::concrete_typename(&arg.ty), used);
                }
            }
        }
        if let MetaType::Interface { possible_types, .. } | MetaType::Union { possible_types, .. } =
            ty
        {
            for name in possible_types {
                self.collect_type(name, used);
            }
        }
        if let MetaType::InputObject { input_fields, .. } = ty {
            for field in input_fields.values() {
                self.collect_type(MetaTypeName::concrete_typename(&field.ty), used);
            }
        }
    }
    fn referenced_types(&self) -> HashSet<&str> {
        let mut used = HashSet::new();
        for directive in self.directives.values() {
            for arg in directive.args.values() {
                self.collect_type(MetaTypeName::concrete_typename(&arg.ty), &mut used);
            }
        }
        for root in Some(&self.query_type)
            .into_iter()
            .chain(self.mutation_type.iter())
            .chain(self.subscription_type.iter())
        {
            self.collect_type(root, &mut used);
        }
        used
    }
    pub(crate) fn introspection_type_names(&self) -> HashSet<&str> {
        let mut used = self.referenced_types();
        for ty in self.types.values() {
            if let MetaType::Interface { possible_types, .. } = ty
                && !used.contains(ty.name())
                && possible_types
                    .iter()
                    .any(|name| used.contains(name.as_str()))
            {
                self.collect_type(ty.name(), &mut used);
            }
        }
        used.extend(
            self.types
                .keys()
                .filter(|name| is_system_type(name))
                .map(String::as_str),
        );
        used
    }
}

fn is_system_type(name: &str) -> bool {
    if name.starts_with("__") {
        return true;
    }
    name == "Boolean" || name == "Int" || name == "Float" || name == "String" || name == "ID"
}
#[cfg(test)]
#[path = "../../../tests/plain-graphql/unit/registry/mod_test.rs"]
mod test;

#[cfg(test)]
#[path = "../../../tests/plain-graphql/unit/registry/reachability_tests.rs"]
mod reachability_tests;
