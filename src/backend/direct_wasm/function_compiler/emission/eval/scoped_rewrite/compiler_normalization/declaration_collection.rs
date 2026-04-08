use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_eval_local_function_bindings(
        program: &Program,
    ) -> HashSet<String> {
        let eval_local_function_names = program
            .functions
            .iter()
            .filter(|function| is_eval_local_function_candidate(function))
            .map(|function| function.name.clone())
            .collect::<HashSet<_>>();
        collect_eval_local_function_declarations(&program.statements, &eval_local_function_names)
            .into_keys()
            .collect::<HashSet<_>>()
    }

    pub(in crate::backend::direct_wasm) fn collect_eval_scoped_declared_bindings(
        program: &Program,
    ) -> HashSet<String> {
        let mut declared_bindings =
            collect_declared_bindings_from_statements_recursive(&program.statements);
        for function in &program.functions {
            declared_bindings.insert(function.name.clone());
            if let Some(binding) = &function.top_level_binding {
                declared_bindings.insert(binding.clone());
            }
            if let Some(binding) = &function.self_binding {
                declared_bindings.insert(binding.clone());
            }
            for parameter in &function.params {
                declared_bindings.insert(parameter.name.clone());
            }
            declared_bindings.extend(collect_declared_bindings_from_statements_recursive(
                &function.body,
            ));
        }
        declared_bindings
    }
}
