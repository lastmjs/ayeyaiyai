use super::*;

impl<'a> FunctionCompiler<'a> {
    fn preserve_identifier_store_global_metadata(
        &mut self,
        name: &str,
        state: &PreparedIdentifierStoreState,
        ensure_descriptor: bool,
    ) -> DirectResult<()> {
        self.update_static_global_assignment_metadata(name, &state.module_assignment_expression);
        self.preserve_exact_static_global_string_binding(
            name,
            state.exact_static_number,
            state.static_string_value.as_ref(),
        );
        self.preserve_static_global_function_binding(name, state.function_binding.as_ref());
        self.preserve_exact_static_global_number_binding(name, &state.module_assignment_expression);
        self.update_global_specialized_function_value(name, &state.module_assignment_expression)?;
        if ensure_descriptor {
            self.ensure_global_property_descriptor_value(
                name,
                &state.module_assignment_expression,
                true,
            );
        } else {
            self.update_global_property_descriptor_value(name, &state.module_assignment_expression);
        }
        Ok(())
    }

    pub(super) fn try_store_identifier_value_via_isolated_indirect_eval_path(
        &mut self,
        name: &str,
        value_local: u32,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<bool> {
        if !self
            .state
            .speculation
            .execution_context
            .isolated_indirect_eval
            || state.resolved_local_binding.is_some()
            || self.parameter_scope_arguments_local_for(name).is_some()
        {
            return Ok(false);
        }

        if let Some(global_index) = self.backend.global_binding_index(name) {
            self.preserve_identifier_store_global_metadata(name, state, false)?;
            self.push_local_get(value_local);
            self.push_global_set(global_index);
            return Ok(true);
        }
        if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
            return Ok(true);
        }
        if let Some(binding) = self.backend.implicit_global_binding(name) {
            self.preserve_identifier_store_global_metadata(name, state, true)?;
            self.emit_store_implicit_global_from_local(binding, value_local)?;
            return Ok(true);
        }
        let binding = self.ensure_implicit_global_binding(name);
        self.preserve_identifier_store_global_metadata(name, state, true)?;
        self.emit_store_implicit_global_from_local(binding, value_local)?;
        Ok(true)
    }

    pub(super) fn store_identifier_value_to_declared_global(
        &mut self,
        name: &str,
        value_local: u32,
        global_index: u32,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<()> {
        if !self
            .state
            .speculation
            .execution_context
            .isolated_indirect_eval
        {
            self.preserve_identifier_store_global_metadata(name, state, false)?;
        }
        self.push_local_get(value_local);
        self.push_global_set(global_index);
        if let Some(array_binding) = state.array_binding.as_ref() {
            self.emit_sync_global_runtime_array_state_from_binding(name, array_binding)?;
        }
        Ok(())
    }

    pub(super) fn store_identifier_value_to_implicit_global(
        &mut self,
        name: &str,
        value_local: u32,
        binding: ImplicitGlobalBinding,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<()> {
        if !self
            .state
            .speculation
            .execution_context
            .isolated_indirect_eval
        {
            self.preserve_identifier_store_global_metadata(name, state, true)?;
        }
        self.emit_store_implicit_global_from_local(binding, value_local)?;
        if let Some(array_binding) = state.array_binding.as_ref() {
            self.emit_sync_global_runtime_array_state_from_binding(name, array_binding)?;
        }
        Ok(())
    }
}
