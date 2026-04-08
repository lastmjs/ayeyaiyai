use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn store_identifier_value_to_resolved_local(
        &mut self,
        name: &str,
        value_local: u32,
        resolved_name: &str,
        local_index: u32,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<()> {
        if !state.is_internal_iterator_temp {
            self.update_local_value_binding(resolved_name, &state.tracked_object_expression);
            self.update_object_prototype_binding_from_value(
                resolved_name,
                &state.tracked_object_expression,
            );
            self.state.speculation.static_semantics.set_local_kind(
                resolved_name,
                state.kind.unwrap_or(StaticValueKind::Unknown),
            );
        }
        self.push_local_get(value_local);
        self.push_local_set(local_index);
        if !state.is_internal_iterator_temp
            && let Some(source_name) = scoped_binding_source_name(name)
            && self
                .resolve_eval_local_function_hidden_name(source_name)
                .is_some()
        {
            self.update_local_value_binding(source_name, &state.tracked_object_expression);
            if let Some(function_binding) = state.function_binding.clone() {
                self.state
                    .speculation
                    .static_semantics
                    .set_local_function_binding(source_name, function_binding);
            } else {
                self.state
                    .speculation
                    .static_semantics
                    .clear_local_function_binding(source_name);
            }
            self.state
                .speculation
                .static_semantics
                .set_local_kind(source_name, state.kind.unwrap_or(StaticValueKind::Unknown));
            self.emit_store_eval_local_function_binding_from_local(source_name, value_local)?;
        }
        Ok(())
    }

    pub(super) fn store_identifier_value_to_eval_local_hidden(
        &mut self,
        name: &str,
        value_local: u32,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<()> {
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
        if let Some(source_name) = scoped_binding_source_name(name) {
            self.update_local_value_binding(source_name, &state.tracked_object_expression);
            self.update_object_prototype_binding_from_value(
                source_name,
                &state.tracked_object_expression,
            );
            if let Some(function_binding) = self
                .state
                .speculation
                .static_semantics
                .local_function_binding(name)
                .cloned()
            {
                self.state
                    .speculation
                    .static_semantics
                    .set_local_function_binding(source_name, function_binding);
            } else {
                self.state
                    .speculation
                    .static_semantics
                    .clear_local_function_binding(source_name);
            }
            self.state
                .speculation
                .static_semantics
                .set_local_kind(source_name, state.kind.unwrap_or(StaticValueKind::Unknown));
        }
        self.emit_store_eval_local_function_binding_from_local(name, value_local)?;
        Ok(())
    }
}
