use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn instantiate_eval_var_bindings(
        &mut self,
        program: &Program,
        preexisting_locals: &HashSet<String>,
    ) -> DirectResult<()> {
        let eval_var_names = collect_eval_var_names(program)
            .into_iter()
            .collect::<BTreeSet<_>>();
        for name in eval_var_names {
            if self.state.speculation.execution_context.top_level_function && !program.strict {
                if !preexisting_locals.contains(&name) {
                    self.state.clear_local_runtime_binding_metadata(&name);
                }
                if self.backend.global_binding_index(&name).is_some() {
                    continue;
                }
                if let Some(binding) = self.backend.implicit_global_binding(&name) {
                    self.ensure_global_property_descriptor_value(
                        &name,
                        &Expression::Undefined,
                        true,
                    );
                    self.push_global_get(binding.present_index);
                    self.state.emission.output.instructions.push(0x45);
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_global_set(binding.value_index);
                    self.push_i32_const(1);
                    self.push_global_set(binding.present_index);
                    self.state.emission.output.instructions.push(0x0b);
                    self.pop_control_frame();
                    continue;
                }
                let binding = self.ensure_implicit_global_binding(&name);
                self.ensure_global_property_descriptor_value(&name, &Expression::Undefined, true);
                let value_local = self.allocate_temp_local();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.emit_store_implicit_global_from_local(binding, value_local)?;
                continue;
            }

            if preexisting_locals.contains(&name) {
                continue;
            }
            let Some((resolved_name, local_index)) = self.resolve_current_local_binding(&name)
            else {
                continue;
            };
            self.state
                .speculation
                .static_semantics
                .set_local_value_binding(&resolved_name, Expression::Undefined);
            self.state
                .speculation
                .static_semantics
                .set_local_kind(&resolved_name, StaticValueKind::Undefined);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(local_index);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn prepare_eval_var_bindings(
        &mut self,
        statements: &mut [Statement],
        strict: bool,
    ) -> DirectResult<()> {
        if !strict {
            return Ok(());
        }

        let var_names = collect_eval_statement_var_names(statements)
            .into_iter()
            .collect::<Vec<_>>();
        if var_names.is_empty() {
            return Ok(());
        }

        let mut renamed_bindings = HashMap::new();
        for name in var_names {
            if renamed_bindings.contains_key(&name) {
                continue;
            }
            let hidden_name =
                self.allocate_named_hidden_local("eval_var", StaticValueKind::Undefined);
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh hidden eval var local must exist");
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(hidden_local);
            renamed_bindings.insert(name, hidden_name);
        }

        for statement in statements {
            self.rewrite_eval_lexical_statement(statement, &renamed_bindings);
        }

        Ok(())
    }
}
