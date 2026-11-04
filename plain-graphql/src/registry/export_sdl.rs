use crate::registry::{Deprecation, MetaField, MetaInputValue, MetaType, Registry};
use std::fmt::Write;
const SYSTEM_SCALARS: &[&str] = &["Int", "Float", "String", "Boolean", "ID"];
impl Registry {
    pub(crate) fn export_sdl(&self) -> String {
        let mut sdl = String::new();
        for ty in self.types.values() {
            if ty.name().starts_with("__") {
                continue;
            }
            self.export_type(ty, &mut sdl);
        }
        self.directives.values().for_each(|directive| {
            if directive.name == "deprecated"
                && !self.types.values().any(|ty| match ty {
                    MetaType::Object { fields, .. } => fields
                        .values()
                        .any(|field| field.deprecation.is_deprecated()),
                    MetaType::Enum { enum_values, .. } => enum_values
                        .values()
                        .any(|value| value.deprecation.is_deprecated()),
                    _ => false,
                })
            {
                return;
            }
            if directive.name == "specifiedBy"
                && !self.types.values().any(|ty| {
                    matches!(
                        ty,
                        MetaType::Scalar {
                            specified_by_url: Some(_),
                            ..
                        }
                    )
                })
            {
                return;
            }
            if directive.name == "oneOf"
                && !self
                    .types
                    .values()
                    .any(|ty| matches!(ty, MetaType::InputObject { oneof: true, .. }))
            {
                return;
            }
            writeln!(sdl, "{}", directive.sdl()).ok();
        });
        {
            writeln!(sdl, "schema {{").ok();
            writeln!(sdl, "{}query: {}", tab(), self.query_type).ok();
            if let Some(mutation_type) = self.mutation_type.as_deref() {
                writeln!(sdl, "{}mutation: {}", tab(), mutation_type).ok();
            }
            if let Some(subscription_type) = self.subscription_type.as_deref() {
                writeln!(sdl, "{}subscription: {}", tab(), subscription_type).ok();
            }
            writeln!(sdl, "}}").ok();
        }
        sdl
    }
    fn export_fields<'a, I: Iterator<Item = &'a MetaField>>(sdl: &mut String, it: I) {
        let fields = it.collect::<Vec<_>>();
        for field in fields {
            if field.name.starts_with("__") || (false) {
                continue;
            }
            if let Some(description) = &field.description {
                write_description(sdl, 1, description);
            }
            if !field.args.is_empty() {
                write!(sdl, "{}{}(", tab(), field.name).ok();
                let args = field.args.values().collect::<Vec<_>>();
                let need_multiline = args.iter().any(|x| x.description.is_some());
                for (i, arg) in args.into_iter().enumerate() {
                    if i != 0 {
                        sdl.push(',');
                    }
                    if let Some(description) = &arg.description {
                        writeln!(sdl).ok();
                        write_description(sdl, 2, description);
                    }
                    if need_multiline {
                        write!(sdl, "{0}{0}", tab()).ok();
                    } else if i != 0 {
                        sdl.push(' ');
                    }
                    write_input_value(sdl, arg);
                }
                if need_multiline {
                    write!(sdl, "\n{}", tab()).ok();
                }
                write!(sdl, "): {}", field.ty).ok();
            } else {
                write!(sdl, "{}{}: {}", tab(), field.name, field.ty).ok();
            }
            write_deprecated(sdl, &field.deprecation);
            writeln!(sdl).ok();
        }
    }
    fn export_type(&self, ty: &MetaType, sdl: &mut String) {
        match ty {
            MetaType::Scalar {
                name, description, ..
            } => {
                let export_scalar = !SYSTEM_SCALARS.contains(&name.as_str());
                if export_scalar {
                    if let Some(description) = description {
                        write_description(sdl, 0, description);
                    }
                    write!(sdl, "scalar {}", name).ok();
                    writeln!(sdl, "\n").ok();
                }
            }
            MetaType::Object {
                name,
                fields,
                description,
                ..
            } => {
                if let Some(description) = description {
                    write_description(sdl, 0, description);
                }
                write!(sdl, "type {}", name).ok();
                self.write_implements(sdl, name);
                writeln!(sdl, " {{").ok();
                Self::export_fields(sdl, fields.values());
                writeln!(sdl, "}}\n").ok();
            }
            MetaType::Interface {
                name,
                fields,
                description,
                ..
            } => {
                if let Some(description) = description {
                    write_description(sdl, 0, description);
                }
                write!(sdl, "interface {}", name).ok();
                self.write_implements(sdl, name);
                writeln!(sdl, " {{").ok();
                Self::export_fields(sdl, fields.values());
                writeln!(sdl, "}}\n").ok();
            }
            MetaType::Enum {
                name,
                enum_values,
                description,
                ..
            } => {
                if let Some(description) = description {
                    write_description(sdl, 0, description);
                }
                write!(sdl, "enum {}", name).ok();
                writeln!(sdl, " {{").ok();
                let values = enum_values.values().collect::<Vec<_>>();
                for value in values {
                    if let Some(description) = &value.description {
                        write_description(sdl, 1, description);
                    }
                    write!(sdl, "{}{}", tab(), value.name).ok();
                    write_deprecated(sdl, &value.deprecation);
                    writeln!(sdl).ok();
                }
                writeln!(sdl, "}}\n").ok();
            }
            MetaType::InputObject {
                name,
                input_fields,
                description,
                oneof,
                ..
            } => {
                if let Some(description) = description {
                    write_description(sdl, 0, description);
                }
                write!(sdl, "input {}", name).ok();
                if *oneof {
                    write!(sdl, " @oneOf").ok();
                }
                writeln!(sdl, " {{").ok();
                let fields = input_fields.values().collect::<Vec<_>>();
                for field in fields {
                    if let Some(description) = &field.description {
                        write_description(sdl, 1, description);
                    }
                    write!(sdl, "{}", tab()).ok();
                    write_input_value(sdl, field);
                    writeln!(sdl).ok();
                }
                writeln!(sdl, "}}\n").ok();
            }
            MetaType::Union {
                name,
                possible_types,
                description,
                ..
            } => {
                if let Some(description) = description {
                    write_description(sdl, 0, description);
                }
                write!(sdl, "union {}", name).ok();
                write!(sdl, " =").ok();
                for (idx, ty) in possible_types.iter().enumerate() {
                    if idx == 0 {
                        write!(sdl, " {}", ty).ok();
                    } else {
                        write!(sdl, " | {}", ty).ok();
                    }
                }
                writeln!(sdl, "\n").ok();
            }
        }
    }
    fn write_implements(&self, sdl: &mut String, name: &str) {
        if let Some(implements) = self.implements.get(name)
            && !implements.is_empty()
        {
            write!(
                sdl,
                " implements {}",
                implements
                    .iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<&str>>()
                    .join(" & ")
            )
            .ok();
        }
    }
}
pub(super) fn write_description(sdl: &mut String, level: usize, description: &str) {
    let tabs = tab().repeat(level);
    {
        let description = description.replace('\n', &format!("\n{tabs}"));
        writeln!(sdl, "{tabs}\"\"\"\n{tabs}{description}\n{tabs}\"\"\"").ok();
    }
}
fn write_input_value(sdl: &mut String, input_value: &MetaInputValue) {
    if let Some(default_value) = &input_value.default_value {
        _ = write!(
            sdl,
            "{}: {} = {}",
            input_value.name, input_value.ty, default_value
        );
    } else {
        _ = write!(sdl, "{}: {}", input_value.name, input_value.ty);
    }
    write_deprecated(sdl, &input_value.deprecation);
}
fn write_deprecated(sdl: &mut String, deprecation: &Deprecation) {
    if let Deprecation::Deprecated { reason } = deprecation {
        let _ = match reason {
            Some(reason) => write!(sdl, " @deprecated(reason: \"{}\")", escape_string(reason)).ok(),
            None => write!(sdl, " @deprecated").ok(),
        };
    }
}
fn escape_string(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() {
        let ec = match c {
            '\\' => Some("\\\\"),
            '\x08' => Some("\\b"),
            '\x0c' => Some("\\f"),
            '\n' => Some("\\n"),
            '\r' => Some("\\r"),
            '\t' => Some("\\t"),
            _ => None,
        };
        match ec {
            Some(ec) => {
                res.write_str(ec).ok();
            }
            None => {
                res.write_char(c).ok();
            }
        }
    }
    res
}
fn tab() -> &'static str {
    "\t"
}
#[cfg(test)]
#[path = "../../../tests/plain-graphql/unit/registry/export_sdl_tests.rs"]
mod tests;
