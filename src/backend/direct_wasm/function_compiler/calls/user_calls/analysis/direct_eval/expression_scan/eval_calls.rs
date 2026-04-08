use super::*;

impl DirectWasmCompiler {
    pub(super) fn collect_static_direct_eval_assigned_nonlocal_names_from_eval_call(
        &self,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        if let Some(CallArgument::Expression(Expression::String(source))) = arguments.first()
            && let Some(mut eval_program) =
                self.parse_static_eval_program_in_context(source, current_function_name)
        {
            namespace_eval_program_internal_function_names(
                &mut eval_program,
                current_function_name,
                source,
            );
            self.normalize_eval_scoped_bindings_to_source_names(&mut eval_program);
            let eval_local_function_declarations = if eval_program.strict {
                HashMap::new()
            } else {
                collect_eval_local_function_declarations(
                    &eval_program.statements,
                    &eval_program
                        .functions
                        .iter()
                        .filter(|function| is_eval_local_function_candidate(function))
                        .map(|function| function.name.clone())
                        .collect::<HashSet<_>>(),
                )
            };
            let mut eval_assigned_names = HashSet::new();
            for statement in eval_program.statements.iter().filter(|statement| {
                !is_eval_local_function_declaration_statement(
                    statement,
                    &eval_local_function_declarations,
                )
            }) {
                collect_assigned_binding_names_from_statement(statement, &mut eval_assigned_names);
                self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                    statement,
                    current_function_name,
                    names,
                );
            }
            let mut declared_bindings =
                collect_declared_bindings_from_statements_recursive(&eval_program.statements);
            for function in &eval_program.functions {
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
            }
            for name in eval_assigned_names {
                let source_name = scoped_binding_source_name(&name).unwrap_or(&name);
                if declared_bindings.contains(source_name) {
                    continue;
                }
                names.insert(source_name.to_string());
            }
        }

        self.collect_static_direct_eval_assigned_nonlocal_names_from_call_arguments(
            arguments,
            current_function_name,
            names,
        );
    }
}
