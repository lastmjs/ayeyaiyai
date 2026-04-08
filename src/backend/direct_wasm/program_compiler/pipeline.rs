use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn compile(
        &mut self,
        program: &Program,
    ) -> DirectResult<Vec<u8>> {
        ProgramCompilationSession::new(self).compile(program)
    }

    pub(in crate::backend::direct_wasm) fn register_functions(
        &mut self,
        functions: &[FunctionDeclaration],
    ) -> DirectResult<()> {
        let function_names = functions
            .iter()
            .map(|function| function.name.clone())
            .collect::<HashSet<_>>();
        for function in functions {
            let arguments_usage = if function.lexical_this {
                ArgumentsUsage::default()
            } else {
                collect_arguments_usage_from_statements(&function.body)
            };
            let extra_argument_indices = arguments_usage
                .indexed_slots
                .into_iter()
                .filter(|index| *index >= function.params.len() as u32)
                .collect::<Vec<_>>();
            let arity = function.params.len() as u32 + 1 + extra_argument_indices.len() as u32;
            let type_index = self.user_type_index_for_arity(arity);
            let user_function = UserFunction {
                name: function.name.clone(),
                kind: function.kind,
                params: function
                    .params
                    .iter()
                    .map(|parameter| parameter.name.clone())
                    .collect(),
                scope_bindings: collect_function_constructor_local_bindings(function),
                parameter_defaults: function
                    .params
                    .iter()
                    .map(|parameter| parameter.default.clone())
                    .collect(),
                body_declares_arguments_binding:
                    collect_declared_bindings_from_statements_recursive(&function.body)
                        .contains("arguments"),
                length: function.length as u32,
                extra_argument_indices,
                enumerated_keys_param_index: collect_enumerated_keys_param_index(function),
                returns_arguments_object: function_returns_arguments_object(&function.body),
                returned_arguments_effects: collect_returned_arguments_effects(&function.body),
                returned_member_function_bindings: collect_returned_member_function_bindings(
                    &function.body,
                    &function_names,
                ),
                returned_member_value_bindings: collect_returned_member_value_bindings(
                    &function.body,
                ),
                inline_summary: collect_inline_function_summary(function),
                home_object_binding: None,
                strict: function.strict,
                lexical_this: function.lexical_this,
                function_index: self.next_user_function_index(),
                type_index,
            };
            self.register_user_function(function.clone(), user_function);
            self.register_returned_function_object_bindings(function);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_returned_function_object_bindings(
        &mut self,
        function: &FunctionDeclaration,
    ) {
        let Some(Expression::Identifier(returned_function_name)) =
            collect_returned_identifier_source_expression(&function.body)
        else {
            return;
        };
        if self.user_function(&returned_function_name).is_none() {
            return;
        }
        let Some(returned_member_value_bindings) = self
            .user_function(&function.name)
            .map(|user_function| user_function.returned_member_value_bindings.clone())
        else {
            return;
        };
        if returned_member_value_bindings.is_empty() {
            return;
        }
        for binding in &returned_member_value_bindings {
            self.define_global_object_property(
                &returned_function_name,
                Expression::String(binding.property.clone()),
                binding.value.clone(),
                true,
            );
        }
    }

    pub(in crate::backend::direct_wasm) fn compile_start(
        &mut self,
        prepared_start: &PreparedStartFunction,
        prepared_inputs: PreparedFunctionCompilerInputs,
    ) -> DirectResult<CompiledFunction> {
        FunctionCompiler::from_prepared_entry_state(
            self,
            prepared_start.entry_state.clone(),
            prepared_inputs,
        )?
        .compile(&prepared_start.statements)
    }

    pub(in crate::backend::direct_wasm) fn prepare_start_statements(
        &self,
        program: &Program,
    ) -> Vec<Statement> {
        let mut start_statements = program
            .functions
            .iter()
            .filter(|function| function.register_global)
            .map(|function| Statement::Assign {
                name: function.name.clone(),
                value: Expression::Identifier(function.name.clone()),
            })
            .collect::<Vec<_>>();
        start_statements.extend_from_slice(&program.statements);
        start_statements
    }

    pub(in crate::backend::direct_wasm) fn compile_user_function(
        &mut self,
        prepared_function: &PreparedUserFunctionCompilation,
        prepared_inputs: PreparedFunctionCompilerInputs,
    ) -> DirectResult<CompiledFunction> {
        let analysis = &prepared_function.analysis;
        let assigned_nonlocal_binding_results =
            prepared_inputs.assigned_nonlocal_binding_results_snapshot();
        let assigned_nonlocal_result_names = assigned_nonlocal_binding_results
            .get(&prepared_function.metadata.name)
            .into_iter()
            .flat_map(|results| results.keys().cloned())
            .collect::<Vec<_>>();
        let compiled = FunctionCompiler::from_prepared_entry_state(
            self,
            prepared_function.entry_state.clone(),
            prepared_inputs,
        )?
        .compile(prepared_function.metadata.body())?;
        debug_assert!(
            assigned_nonlocal_result_names
                .iter()
                .all(|name| analysis.assigned_nonlocal_bindings.contains(name))
        );
        Ok(compiled)
    }
}
