use proc_macro2::{Span, TokenStream};
use syn::{
    Attribute, DeriveInput, Error, Expr, Field, Lit, Meta, Path, Result, Type, Variant,
    parse::Parser, punctuated::Punctuated, token::Comma,
};

pub trait FromMeta: Sized {
    fn from_meta(meta: &Meta) -> Result<Self> {
        match meta {
            Meta::List(list) => Self::from_list(parse_list(list.tokens.clone())?),
            _ => Err(Error::new_spanned(
                meta,
                "Expected a list of GraphQL options",
            )),
        }
    }

    fn from_list(items: Vec<Meta>) -> Result<Self> {
        let span = items
            .first()
            .map_or_else(Span::call_site, |item| syn::spanned::Spanned::span(item));
        Err(Error::new(span, "Unexpected nested GraphQL options"))
    }
}

pub fn parse_list(tokens: TokenStream) -> Result<Vec<Meta>> {
    Ok(Punctuated::<Meta, Comma>::parse_terminated
        .parse2(tokens)?
        .into_iter()
        .collect())
}

pub fn literal(meta: &Meta) -> Result<&Lit> {
    match expression(meta)? {
        Expr::Lit(lit) => Ok(&lit.lit),
        _ => Err(Error::new_spanned(meta, "Expected a literal value")),
    }
}

fn expression(meta: &Meta) -> Result<&Expr> {
    let Meta::NameValue(value) = meta else {
        return Err(Error::new_spanned(meta, "Expected an option value"));
    };
    let mut expression = &value.value;
    while let Expr::Group(group) = expression {
        expression = &group.expr;
    }
    Ok(expression)
}

pub fn string(meta: &Meta) -> Result<&syn::LitStr> {
    match literal(meta)? {
        Lit::Str(value) => Ok(value),
        _ => Err(Error::new_spanned(meta, "Expected a string")),
    }
}

impl FromMeta for String {
    fn from_meta(meta: &Meta) -> Result<Self> {
        Ok(string(meta)?.value())
    }
}

impl FromMeta for bool {
    fn from_meta(meta: &Meta) -> Result<Self> {
        if matches!(meta, Meta::Path(_)) {
            return Ok(true);
        }
        match literal(meta)? {
            Lit::Bool(value) => Ok(value.value),
            Lit::Str(value) => value
                .value()
                .parse()
                .map_err(|error| Error::new(value.span(), error)),
            _ => Err(Error::new_spanned(meta, "Expected a boolean")),
        }
    }
}

impl FromMeta for usize {
    fn from_meta(meta: &Meta) -> Result<Self> {
        match literal(meta)? {
            Lit::Int(value) => value.base10_parse(),
            Lit::Str(value) => value
                .value()
                .parse()
                .map_err(|error| Error::new(value.span(), error)),
            _ => Err(Error::new_spanned(meta, "Expected an unsigned integer")),
        }
    }
}

impl FromMeta for Expr {
    fn from_meta(meta: &Meta) -> Result<Self> {
        match expression(meta)? {
            Expr::Lit(syn::ExprLit {
                lit: Lit::Str(text),
                ..
            }) => text.parse(),
            value => Ok(value.clone()),
        }
    }
}

impl FromMeta for Path {
    fn from_meta(meta: &Meta) -> Result<Self> {
        match expression(meta)? {
            Expr::Path(value) => Ok(value.path.clone()),
            _ => string(meta)?.parse(),
        }
    }
}

impl FromMeta for Type {
    fn from_meta(meta: &Meta) -> Result<Self> {
        string(meta)?.parse()
    }
}

pub struct Options(Vec<Meta>);

impl Options {
    pub fn new(items: Vec<Meta>) -> Self {
        Self(items)
    }

    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut items = Vec::new();
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("graphql")) {
            match &attr.meta {
                Meta::List(list) => items.extend(parse_list(list.tokens.clone())?),
                Meta::Path(_) => {}
                _ => return Err(Error::new_spanned(attr, "Expected GraphQL options")),
            }
        }
        Ok(Self(items))
    }

    pub fn optional<T: FromMeta>(&mut self, name: &str) -> Result<Option<T>> {
        let mut matches = self
            .0
            .iter()
            .enumerate()
            .filter(|(_, item)| item.path().is_ident(name));
        let Some((index, _)) = matches.next() else {
            return Ok(None);
        };
        if let Some((_, duplicate)) = matches.next() {
            return Err(Error::new_spanned(
                duplicate,
                format!("Duplicate GraphQL option `{name}`"),
            ));
        }
        T::from_meta(&self.0.remove(index)).map(Some)
    }

    pub fn required<T: FromMeta>(&mut self, name: &str) -> Result<T> {
        self.optional(name)?.ok_or_else(|| {
            Error::new(
                Span::call_site(),
                format!("Missing GraphQL option `{name}`"),
            )
        })
    }

    pub fn multiple<T: FromMeta>(&mut self, name: &str) -> Result<Vec<T>> {
        let (items, rest) = std::mem::take(&mut self.0)
            .into_iter()
            .partition::<Vec<_>, _>(|item| item.path().is_ident(name));
        self.0 = rest;
        items.iter().map(T::from_meta).collect()
    }

    pub fn finish(self) -> Result<()> {
        match self.0.first() {
            Some(item) => Err(Error::new_spanned(item.path(), "Unknown GraphQL option")),
            None => Ok(()),
        }
    }
}

pub enum Data<V, F> {
    Enum(Vec<V>),
    Struct(Fields<F>),
}

impl<V, F> Data<V, F> {
    pub fn parse(
        input: &DeriveInput,
        variant: impl FnMut(&Variant) -> Result<V>,
        field: impl FnMut(&Field) -> Result<F>,
    ) -> Result<Self> {
        match &input.data {
            syn::Data::Enum(data) => Ok(Self::Enum(
                data.variants.iter().map(variant).collect::<Result<_>>()?,
            )),
            syn::Data::Struct(data) => Fields::parse(&data.fields, field).map(Self::Struct),
            syn::Data::Union(_) => Err(Error::new_spanned(input, "Unsupported Rust union")),
        }
    }
}

pub enum Style {
    Struct,
    Tuple,
    Unit,
}

pub struct Fields<T> {
    pub style: Style,
    pub fields: Vec<T>,
}

impl<T> Fields<T> {
    pub fn parse(fields: &syn::Fields, field: impl FnMut(&Field) -> Result<T>) -> Result<Self> {
        let style = match fields {
            syn::Fields::Named(_) => Style::Struct,
            syn::Fields::Unnamed(_) => Style::Tuple,
            syn::Fields::Unit => Style::Unit,
        };
        let fields = fields.iter().map(field).collect::<Result<_>>()?;
        Ok(Self { style, fields })
    }
}

pub fn docs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .cloned()
        .collect()
}
