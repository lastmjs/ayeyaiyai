use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn snapshot_bound_capture_slot_expression(
        &self,
        slot_name: &str,
    ) -> Expression {
        if let Some(array_binding) = self
            .resolve_array_binding_from_expression(&Expression::Identifier(slot_name.to_string()))
        {
            return Expression::Array(
                array_binding
                    .values
                    .iter()
                    .map(|value| {
                        ArrayElement::Expression(value.clone().unwrap_or(Expression::Undefined))
                    })
                    .collect(),
            );
        }
        if let Some(object_binding) = self
            .resolve_object_binding_from_expression(&Expression::Identifier(slot_name.to_string()))
        {
            return object_binding_to_expression(&object_binding);
        }
        let identifier = Expression::Identifier(slot_name.to_string());
        if let Some(value) = self
            .resolve_bound_alias_expression(&identifier)
            .filter(|value| !static_expression_matches(value, &identifier))
        {
            return self.materialize_static_expression(&value);
        }
        identifier
    }

    pub(in crate::backend::direct_wasm) fn prepare_bound_user_function_capture_bindings(
        &mut self,
        user_function: &UserFunction,
        capture_slots: &BTreeMap<String, String>,
    ) -> DirectResult<Vec<PreparedBoundCaptureBinding>> {
        let Some(capture_bindings) = self.user_function_capture_bindings(&user_function.name)
        else {
            return Ok(Vec::new());
        };

        let mut prepared = Vec::new();
        for (capture_name, capture_hidden_name) in capture_bindings {
            let Some(slot_name) = capture_slots.get(&capture_name) else {
                continue;
            };
            let Some(slot_local) = self.state.runtime.locals.bindings.get(slot_name).copied()
            else {
                continue;
            };
            let source_binding_name = self
                .state
                .speculation
                .static_semantics
                .capture_slot_source_bindings
                .get(slot_name)
                .cloned()
                .or_else(|| {
                    self.state
                        .speculation
                        .static_semantics
                        .local_value_binding(slot_name)
                        .and_then(|value| {
                            let Expression::Identifier(name) =
                                self.materialize_static_expression(value)
                            else {
                                return None;
                            };
                            Some(name)
                        })
                });
            let binding = self
                .implicit_global_binding(&capture_hidden_name)
                .unwrap_or_else(|| self.ensure_implicit_global_binding(&capture_hidden_name));
            let saved_value_local = self.allocate_temp_local();
            let saved_present_local = self.allocate_temp_local();
            self.push_global_get(binding.value_index);
            self.push_local_set(saved_value_local);
            self.push_global_get(binding.present_index);
            self.push_local_set(saved_present_local);
            prepared.push(PreparedBoundCaptureBinding {
                binding,
                capture_name,
                capture_hidden_name,
                slot_name: slot_name.clone(),
                source_binding_name,
                slot_local,
                saved_value_local,
                saved_present_local,
            });
        }

        Ok(prepared)
    }

    pub(in crate::backend::direct_wasm) fn emit_prepare_bound_user_function_capture_globals(
        &mut self,
        prepared: &[PreparedBoundCaptureBinding],
    ) -> DirectResult<()> {
        for binding in prepared {
            self.sync_user_function_capture_static_metadata(
                &binding.slot_name,
                &binding.capture_hidden_name,
            );
            self.push_local_get(binding.slot_local);
            self.push_global_set(binding.binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(binding.binding.present_index);
            if let Some(source_binding_name) = binding.source_binding_name.as_ref() {
                self.emit_runtime_object_property_shadow_copy(
                    source_binding_name,
                    &binding.capture_hidden_name,
                )?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn sync_bound_user_function_capture_slots(
        &mut self,
        prepared: &[PreparedBoundCaptureBinding],
        updated_bindings: Option<&HashMap<String, Expression>>,
    ) -> DirectResult<()> {
        for binding in prepared {
            let value_local = self.allocate_temp_local();
            self.push_global_get(binding.binding.value_index);
            self.push_local_set(value_local);
            if let Some(value) =
                updated_bindings.and_then(|bindings| bindings.get(&binding.capture_name))
            {
                self.update_capture_slot_binding_from_expression(&binding.slot_name, value)?;
                if let Some(source_binding_name) = &binding.source_binding_name {
                    self.sync_bound_capture_source_binding_metadata(source_binding_name, value)?;
                    self.emit_runtime_object_property_shadow_copy(
                        &binding.capture_hidden_name,
                        source_binding_name,
                    )?;
                }
            } else {
                self.update_capture_slot_binding_from_expression(
                    &binding.slot_name,
                    &Expression::Identifier(binding.capture_hidden_name.clone()),
                )?;
            }
            self.push_local_get(value_local);
            self.push_local_set(binding.slot_local);
            if let Some(source_binding_name) = binding.source_binding_name.as_ref() {
                self.emit_sync_identifier_runtime_value_from_local(
                    source_binding_name,
                    value_local,
                )?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn sync_bound_capture_source_binding_metadata(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        let is_local_binding = self.parameter_scope_arguments_local_for(name).is_some()
            || self.resolve_current_local_binding(name).is_some()
            || self.resolve_eval_local_function_hidden_name(name).is_some()
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some();
        if is_local_binding {
            self.update_capture_slot_binding_from_expression(name, value)?;
        } else {
            self.state.clear_local_static_binding_metadata(name);
        }

        if !is_local_binding
            && (self
                .backend
                .global_semantics
                .names
                .bindings
                .contains_key(name)
                || self.global_has_implicit_binding(name))
        {
            self.update_static_global_assignment_metadata(name, value);
            self.update_global_specialized_function_value(name, value)?;
            self.update_global_property_descriptor_value(name, value);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn restore_bound_user_function_capture_bindings(
        &mut self,
        prepared: &[PreparedBoundCaptureBinding],
    ) {
        for binding in prepared.iter().rev() {
            self.push_local_get(binding.saved_value_local);
            self.push_global_set(binding.binding.value_index);
            self.push_local_get(binding.saved_present_local);
            self.push_global_set(binding.binding.present_index);
        }
    }
}
