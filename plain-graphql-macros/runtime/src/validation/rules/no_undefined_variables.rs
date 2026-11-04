use crate::value::Value;
use crate::{
    Name, Pos, Positioned,
    parser::types::{
        ExecutableDocument, FragmentDefinition, FragmentSpread, OperationDefinition,
        VariableDefinition,
    },
    validation::{
        utils::{Scope, referenced_variables},
        visitor::{Visitor, VisitorContext},
    },
};
use std::collections::{HashMap, HashSet};
#[derive(Default)]
pub struct NoUndefinedVariables<'a> {
    defined_variables: HashMap<Option<&'a str>, (Pos, HashSet<&'a str>)>,
    used_variables: HashMap<Scope<'a>, HashMap<&'a str, Pos>>,
    current_scope: Option<Scope<'a>>,
    spreads: HashMap<Scope<'a>, Vec<&'a str>>,
}
impl<'a> NoUndefinedVariables<'a> {
    fn find_undef_vars(
        &'a self,
        scope: &Scope<'a>,
        defined: &HashSet<&'a str>,
        undef: &mut Vec<(&'a str, Pos)>,
        visited: &mut HashSet<Scope<'a>>,
    ) {
        if visited.contains(scope) {
            return;
        }
        visited.insert(*scope);
        if let Some(used_vars) = self.used_variables.get(scope) {
            for (var, pos) in used_vars {
                if !defined.contains(var) {
                    undef.push((*var, *pos));
                }
            }
        }
        if let Some(spreads) = self.spreads.get(scope) {
            for spread in spreads {
                self.find_undef_vars(&Scope::Fragment(spread), defined, undef, visited);
            }
        }
    }
}
impl<'a> Visitor<'a> for NoUndefinedVariables<'a> {
    fn exit_document(&mut self, ctx: &mut VisitorContext<'a>, _doc: &'a ExecutableDocument) {
        for (op_name, (def_pos, def_vars)) in &self.defined_variables {
            let mut undef = Vec::new();
            let mut visited = HashSet::new();
            self.find_undef_vars(
                &Scope::Operation(*op_name),
                def_vars,
                &mut undef,
                &mut visited,
            );
            for (var, pos) in undef {
                if let Some(op_name) = op_name {
                    ctx.report_error(
                        vec![*def_pos, pos],
                        format!(
                            r#"Variable "${}" is not defined by operation "{}""#,
                            var, op_name
                        ),
                    );
                } else {
                    ctx.report_error(vec![pos], format!(r#"Variable "${}" is not defined"#, var));
                }
            }
        }
    }
    fn enter_operation_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        name: Option<&'a Name>,
        operation_definition: &'a Positioned<OperationDefinition>,
    ) {
        let name = name.map(Name::as_str);
        self.current_scope = Some(Scope::Operation(name));
        self.defined_variables
            .insert(name, (operation_definition.pos, HashSet::new()));
    }
    fn enter_fragment_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        name: &'a Name,
        _fragment_definition: &'a Positioned<FragmentDefinition>,
    ) {
        self.current_scope = Some(Scope::Fragment(name));
    }
    fn enter_variable_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        variable_definition: &'a Positioned<VariableDefinition>,
    ) {
        if let Some(Scope::Operation(ref name)) = self.current_scope
            && let Some(&mut (_, ref mut vars)) = self.defined_variables.get_mut(name)
        {
            vars.insert(&variable_definition.node.name.node);
        }
    }
    fn enter_argument(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        name: &'a Positioned<Name>,
        value: &'a Positioned<Value>,
    ) {
        if let Some(ref scope) = self.current_scope {
            self.used_variables.entry(*scope).or_default().extend(
                referenced_variables(&value.node)
                    .into_iter()
                    .map(|n| (n, name.pos)),
            );
        }
    }
    fn enter_fragment_spread(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        fragment_spread: &'a Positioned<FragmentSpread>,
    ) {
        if let Some(ref scope) = self.current_scope {
            self.spreads
                .entry(*scope)
                .or_default()
                .push(&fragment_spread.node.fragment_name.node);
        }
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/no_undefined_variables_tests.rs"]
mod tests;
