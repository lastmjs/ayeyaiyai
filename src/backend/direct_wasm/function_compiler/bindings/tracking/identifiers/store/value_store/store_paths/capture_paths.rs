use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn store_identifier_value_to_capture_binding(
        &mut self,
        name: &str,
        value_local: u32,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<()> {
        if !state.is_internal_iterator_temp {
            self.update_local_value_binding(name, &state.tracked_object_expression);
            self.update_object_prototype_binding_from_value(name, &state.tracked_object_expression);
            if let Some(function_binding) = state.function_binding.clone() {
                self.state
                    .speculation
                    .static_semantics
                    .set_local_function_binding(name, function_binding);
            } else {
                self.state
                    .speculation
                    .static_semantics
                    .clear_local_function_binding(name);
            }
            self.state
                .speculation
                .static_semantics
                .set_local_kind(name, state.kind.unwrap_or(StaticValueKind::Unknown));
        }
        if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
            && !self
                .state
                .speculation
                .execution_context
                .isolated_indirect_eval
        {
            self.update_static_global_assignment_metadata(
                &hidden_name,
                &state.module_assignment_expression,
            );
            self.preserve_exact_static_global_number_binding(
                &hidden_name,
                &state.module_assignment_expression,
            );
            self.update_global_specialized_function_value(
                &hidden_name,
                &state.module_assignment_expression,
            )?;
            self.ensure_global_property_descriptor_value(
                &hidden_name,
                &state.module_assignment_expression,
                true,
            );
        }
        self.emit_store_user_function_capture_binding_from_local(name, value_local)?;
        Ok(())
    }
}
