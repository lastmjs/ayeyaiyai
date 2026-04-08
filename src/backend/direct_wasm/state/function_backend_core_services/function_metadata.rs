use super::super::*;

impl<'a> FunctionCompilerBackend<'a> {
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
