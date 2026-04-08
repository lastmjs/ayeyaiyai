use crate::backend::direct_wasm::{
    CompilerState, UserFunction, collect_assigned_binding_names_from_statement,
    scoped_binding_source_name,
};
use std::collections::HashSet;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.module_artifacts.reset_for_program();
        self.function_registry.reset_for_program();
        self.global_semantics.reset_for_program();
        self.test262.reset_for_program();
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_assigned_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        let Some(function) = self
            .function_registry
            .registered_function(&user_function.name)
        else {
            return HashSet::new();
        };
        let mut names = HashSet::new();
        for statement in &function.body {
            collect_assigned_binding_names_from_statement(statement, &mut names);
        }
        names.retain(|name| {
            let source_name = scoped_binding_source_name(name).unwrap_or(name);
            !user_function.scope_bindings.contains(source_name)
        });
        names
    }
}
