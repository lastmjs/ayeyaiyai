use super::*;

mod expression_traversal;
mod statement_traversal;

use statement_traversal::collect_updated_names_from_statement;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_user_function_updated_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let mut names: HashSet<String> = HashSet::new();
        for statement in &function.body {
            collect_updated_names_from_statement(statement, &mut names);
        }
        names.retain(|name| {
            let source_name = scoped_binding_source_name(name).unwrap_or(name);
            !user_function.scope_bindings.contains(source_name)
        });
        names
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_assigned_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        self.backend
            .collect_user_function_assigned_nonlocal_bindings(user_function)
    }

    pub(in crate::backend::direct_wasm) fn invalidate_user_function_assigned_nonlocal_bindings(
        &mut self,
        user_function: &UserFunction,
    ) {
        let names = self.collect_user_function_call_effect_nonlocal_bindings(user_function);
        if !names.is_empty() {
            self.invalidate_static_binding_metadata_for_names(&names);
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_snapshot_updated_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
        updated_bindings: Option<&HashMap<String, Expression>>,
    ) -> HashSet<String> {
        let mut names = HashSet::new();
        let Some(updated_bindings) = updated_bindings else {
            return names;
        };
        for name in updated_bindings.keys() {
            let source_name = scoped_binding_source_name(name).unwrap_or(name).to_string();
            if source_name == "this" || source_name == "arguments" {
                continue;
            }
            if user_function.scope_bindings.contains(&source_name) {
                continue;
            }
            names.insert(source_name);
        }
        names
    }
}
