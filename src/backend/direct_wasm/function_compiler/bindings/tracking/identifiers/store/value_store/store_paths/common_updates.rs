use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn apply_identifier_store_shared_updates(
        &mut self,
        value_local: u32,
        state: &PreparedIdentifierStoreState,
    ) -> DirectResult<()> {
        self.update_member_function_bindings_for_value(
            &state.resolved_name,
            &state.canonical_value_expression,
            value_local,
        )?;
        if !state.is_internal_iterator_temp {
            let specialized_value_expression = match &state.canonical_value_expression {
                Expression::Member { object, property }
                    if self
                        .resolve_member_getter_binding(object, property)
                        .is_some() =>
                {
                    &state.canonical_value_expression
                }
                _ => &state.tracked_value_expression,
            };
            self.update_local_function_binding(
                &state.resolved_name,
                &state.function_binding_expression,
            );
            self.update_local_specialized_function_value(
                &state.resolved_name,
                specialized_value_expression,
            )?;
            self.update_local_proxy_binding(&state.resolved_name, &state.tracked_value_expression);
            if !(matches!(
                state.canonical_value_expression,
                Expression::Call { .. } | Expression::New { .. }
            ) && matches!(state.tracked_value_expression, Expression::Object(_)))
            {
                self.update_object_literal_member_bindings_for_value(
                    &state.resolved_name,
                    &state.tracked_object_expression,
                );
            }
            self.update_local_array_binding(&state.resolved_name, &state.tracked_value_expression);
            self.update_local_resizable_array_buffer_binding(
                &state.resolved_name,
                &state.tracked_value_expression,
            )?;
            self.update_local_typed_array_view_binding(
                &state.resolved_name,
                &state.tracked_value_expression,
            )?;
        }
        self.update_local_array_iterator_binding_with_source(
            &state.resolved_name,
            state.iterator_binding_source.clone(),
        );
        self.update_local_iterator_step_binding(
            &state.resolved_name,
            &state.tracked_value_expression,
        );
        if state.is_internal_array_step_binding {
            self.state
                .speculation
                .static_semantics
                .set_local_kind(&state.resolved_name, StaticValueKind::Object);
        }
        if !state.is_internal_array_iterator_binding {
            self.update_local_object_binding(
                &state.resolved_name,
                &state.object_binding_expression,
            );
        }
        if !state.is_internal_iterator_temp {
            self.update_local_arguments_binding(
                &state.resolved_name,
                &state.tracked_value_expression,
            );
            self.update_local_descriptor_binding(
                &state.resolved_name,
                &state.descriptor_binding_expression,
            );
            if let Some(descriptor) = state.returned_descriptor_binding.clone() {
                self.state
                    .speculation
                    .static_semantics
                    .objects
                    .local_descriptor_bindings
                    .insert(state.resolved_name.clone(), descriptor);
            }
        }
        Ok(())
    }
}
