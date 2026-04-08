use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_eval_local_function_bindings(
        &mut self,
        current_function_name: Option<&str>,
        program: &Program,
    ) {
        let Some(current_function_name) = current_function_name else {
            return;
        };
        let Some(current_function) = self.user_function(current_function_name) else {
            return;
        };
        let current_function_strict = current_function.strict;
        let current_function_scope_bindings = current_function.scope_bindings.clone();
        if current_function_strict || program.strict {
            return;
        }

        let local_function_names = program
            .functions
            .iter()
            .filter(|function| is_eval_local_function_candidate(function))
            .map(|function| function.name.clone())
            .collect::<HashSet<_>>();
        if local_function_names.is_empty() {
            return;
        }

        let bindings =
            collect_eval_local_function_declarations(&program.statements, &local_function_names)
                .into_keys()
                .collect::<Vec<_>>();
        if bindings.is_empty() {
            return;
        }

        let target_function_names = std::iter::once(current_function_name.to_string())
            .chain(
                program
                    .functions
                    .iter()
                    .map(|function| function.name.clone()),
            )
            .collect::<Vec<_>>();

        for binding_name in bindings {
            if current_function_scope_bindings.contains(&binding_name) {
                continue;
            }
            let hidden_name = format!(
                "__ayy_eval_local_fn_binding__{}__{}",
                current_function_name, binding_name
            );
            self.ensure_implicit_global_binding(&hidden_name);
            for function_name in &target_function_names {
                self.record_eval_local_function_binding(function_name, &binding_name, &hidden_name);
            }
        }
    }
}
