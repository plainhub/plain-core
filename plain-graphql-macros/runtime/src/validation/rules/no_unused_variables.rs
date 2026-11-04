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
pub struct NoUnusedVariables<'a> {
    defined_variables: HashMap<Option<&'a str>, HashSet<(&'a str, Pos)>>,
    used_variables: HashMap<Scope<'a>, Vec<&'a str>>,
    current_scope: Option<Scope<'a>>,
    spreads: HashMap<Scope<'a>, Vec<&'a str>>,
}
impl<'a> NoUnusedVariables<'a> {
    fn find_used_vars(
        &self,
        from: &Scope<'a>,
        defined: &HashSet<&'a str>,
        used: &mut HashSet<&'a str>,
        visited: &mut HashSet<Scope<'a>>,
    ) {
        if visited.contains(from) {
            return;
        }
        visited.insert(*from);
        if let Some(used_vars) = self.used_variables.get(from) {
            for var in used_vars {
                if defined.contains(var) {
                    used.insert(var);
                }
            }
        }
        if let Some(spreads) = self.spreads.get(from) {
            for spread in spreads {
                self.find_used_vars(&Scope::Fragment(spread), defined, used, visited);
            }
        }
    }
}
impl<'a> Visitor<'a> for NoUnusedVariables<'a> {
    fn exit_document(&mut self, ctx: &mut VisitorContext<'a>, _doc: &'a ExecutableDocument) {
        for (op_name, def_vars) in &self.defined_variables {
            let mut used = HashSet::new();
            let mut visited = HashSet::new();
            self.find_used_vars(
                &Scope::Operation(*op_name),
                &def_vars.iter().map(|(name, _)| *name).collect(),
                &mut used,
                &mut visited,
            );
            for (var, pos) in def_vars.iter().filter(|(var, _)| !used.contains(var)) {
                if let Some(op_name) = op_name {
                    ctx.report_error(
                        vec![*pos],
                        format!(
                            r#"Variable "${}" is not used by operation "{}""#,
                            var, op_name
                        ),
                    );
                } else {
                    ctx.report_error(vec![*pos], format!(r#"Variable "${}" is not used"#, var));
                }
            }
        }
    }
    fn enter_operation_definition(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        name: Option<&'a Name>,
        _operation_definition: &'a Positioned<OperationDefinition>,
    ) {
        let op_name = name.map(Name::as_str);
        self.current_scope = Some(Scope::Operation(op_name));
        self.defined_variables.insert(op_name, HashSet::new());
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
            && let Some(vars) = self.defined_variables.get_mut(name)
        {
            vars.insert((&variable_definition.node.name.node, variable_definition.pos));
        }
    }
    fn enter_argument(
        &mut self,
        _ctx: &mut VisitorContext<'a>,
        _name: &'a Positioned<Name>,
        value: &'a Positioned<Value>,
    ) {
        if let Some(ref scope) = self.current_scope {
            self.used_variables
                .entry(*scope)
                .or_default()
                .append(&mut referenced_variables(&value.node));
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
#[path = "../../../../../tests/plain-graphql/unit/validation/rules/no_unused_variables_tests.rs"]
mod tests;
