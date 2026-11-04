use crate::{
    Pos, Positioned, ServerError, ServerResult, Value,
    parser::types::{Directive, ExecutableDocument, OperationType, Selection, SelectionSet},
    registry::{MetaType, MetaTypeName, Registry},
};

pub(crate) fn validate(registry: &Registry, document: &ExecutableDocument) -> ServerResult<()> {
    for (_, operation) in document.operations.iter() {
        for variable in &operation.node.variable_definitions {
            if let Some(default) = &variable.node.default_value {
                input(
                    registry,
                    &variable.node.var_type.node.to_string(),
                    &default.node,
                    default.pos,
                )?;
            }
            directives(registry, &variable.node.directives)?;
        }
        directives(registry, &operation.node.directives)?;
        let root = match operation.node.ty {
            OperationType::Query => Some(&registry.query_type),
            OperationType::Mutation => registry.mutation_type.as_ref(),
            OperationType::Subscription => registry.subscription_type.as_ref(),
        };
        selections(
            registry,
            root.map(String::as_str),
            &operation.node.selection_set.node,
        )?;
    }
    for fragment in document.fragments.values() {
        directives(registry, &fragment.node.directives)?;
        selections(
            registry,
            Some(fragment.node.type_condition.node.on.node.as_str()),
            &fragment.node.selection_set.node,
        )?;
    }
    Ok(())
}

fn selections(
    registry: &Registry,
    parent: Option<&str>,
    selections: &SelectionSet,
) -> ServerResult<()> {
    for selection in &selections.items {
        directives(registry, selection.node.directives())?;
        match &selection.node {
            Selection::Field(field) => {
                if let Some(metadata) = parent
                    .and_then(|name| registry.types.get(name))
                    .and_then(|ty| ty.field_by_name(field.node.name.node.as_str()))
                {
                    for (name, value) in &field.node.arguments {
                        if let Some(argument) = metadata.args.get(name.node.as_str()) {
                            let literal = value
                                .node
                                .clone()
                                .into_const_with::<std::convert::Infallible>(|_| Ok(Value::Null))
                                .unwrap();
                            input(registry, &argument.ty, &literal, value.pos)?;
                        }
                    }
                    self::selections(
                        registry,
                        Some(MetaTypeName::concrete_typename(&metadata.ty)),
                        &field.node.selection_set.node,
                    )?;
                }
            }
            Selection::InlineFragment(fragment) => {
                let parent = fragment
                    .node
                    .type_condition
                    .as_ref()
                    .map(|condition| condition.node.on.node.as_str())
                    .or(parent);
                self::selections(registry, parent, &fragment.node.selection_set.node)?;
            }
            Selection::FragmentSpread(_) => {}
        }
    }
    Ok(())
}

fn directives(registry: &Registry, directives: &[Positioned<Directive>]) -> ServerResult<()> {
    for directive in directives {
        if let Some(metadata) = registry.directives.get(directive.node.name.node.as_str()) {
            for (name, value) in &directive.node.arguments {
                if let Some(argument) = metadata.args.get(name.node.as_str()) {
                    let literal = value
                        .node
                        .clone()
                        .into_const_with::<std::convert::Infallible>(|_| Ok(Value::Null))
                        .unwrap();
                    input(registry, &argument.ty, &literal, value.pos)?;
                }
            }
        }
    }
    Ok(())
}

fn input(registry: &Registry, expected: &str, value: &Value, position: Pos) -> ServerResult<()> {
    match MetaTypeName::create(expected) {
        MetaTypeName::NonNull(inner) => input(registry, inner, value, position),
        MetaTypeName::List(inner) => {
            if let Value::List(values) = value {
                for value in values {
                    input(registry, inner, value, position)?;
                }
                Ok(())
            } else {
                input(registry, inner, value, position)
            }
        }
        MetaTypeName::Named(name) => match (registry.types.get(name), value) {
            (Some(MetaType::Enum { .. }), Value::String(_)) => Err(ServerError::new(
                format!("Enum \"{name}\" cannot represent a string literal."),
                Some(position),
            )),
            (Some(MetaType::InputObject { input_fields, .. }), Value::Object(values)) => {
                for (name, value) in values {
                    if let Some(field) = input_fields.get(name.as_str()) {
                        input(registry, &field.ty, value, position)?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        },
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-graphql/unit/validation/enum_literals.rs"]
mod tests;
