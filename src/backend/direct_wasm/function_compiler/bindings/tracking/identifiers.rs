use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn is_identifier_bound(&self, name: &str) -> bool {
        self.lookup_identifier_kind(name).is_some()
    }

    pub(in crate::backend::direct_wasm) fn is_unshadowed_builtin_identifier(
        &self,
        name: &str,
    ) -> bool {
        self.resolve_current_local_binding(name).is_none()
            && !self.module.global_bindings.contains_key(name)
            && !self.module.global_function_bindings.contains_key(name)
            && !is_internal_user_function_identifier(name)
    }

    pub(in crate::backend::direct_wasm) fn emit_store_identifier_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<()> {
        let resolved_local_binding = self.resolve_current_local_binding(name);
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(value_local);
            self.push_local_set(parameter_scope_arguments_local);
        }
        if let Some((_, local_index)) = resolved_local_binding {
            self.push_local_get(value_local);
            self.push_local_set(local_index);
        } else if let Some(global_index) = self.module.global_bindings.get(name).copied() {
            self.push_local_get(value_local);
            self.push_global_set(global_index);
        } else if self.emit_store_user_function_capture_binding_from_local(name, value_local)? {
        } else if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
        } else if let Some(binding) = self.module.implicit_global_bindings.get(name).copied() {
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        } else {
            let binding = self.module.ensure_implicit_global_binding(name);
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_sync_identifier_runtime_value_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<()> {
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(value_local);
            self.push_local_set(parameter_scope_arguments_local);
        }
        if let Some((_, local_index)) = self.resolve_current_local_binding(name) {
            self.push_local_get(value_local);
            self.push_local_set(local_index);
        } else if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) {
            self.sync_user_function_capture_static_metadata(name, &hidden_name);
            self.emit_store_user_function_capture_binding_from_local(name, value_local)?;
        } else if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
        } else if let Some(global_index) = self.module.global_bindings.get(name).copied() {
            self.push_local_get(value_local);
            self.push_global_set(global_index);
        } else if let Some(binding) = self.module.implicit_global_bindings.get(name).copied() {
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        } else {
            let binding = self.module.ensure_implicit_global_binding(name);
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_store_identifier_value_local(
        &mut self,
        name: &str,
        value_expression: &Expression,
        value_local: u32,
    ) -> DirectResult<()> {
        self.deleted_builtin_identifiers.remove(name);
        let canonical_value_expression = self
            .prepare_special_assignment_expression(value_expression)
            .unwrap_or_else(|| value_expression.clone());
        let function_binding =
            self.resolve_function_binding_from_expression(&canonical_value_expression);
        let kind = self.infer_value_kind(&canonical_value_expression);
        let static_string_value = if kind == Some(StaticValueKind::String) {
            self.resolve_static_string_value(&canonical_value_expression)
        } else {
            None
        };
        let exact_static_number = self
            .resolve_static_number_value(&canonical_value_expression)
            .filter(|number| {
                number.is_nan()
                    || !number.is_finite()
                    || number.fract() != 0.0
                    || (*number == 0.0 && number.is_sign_negative())
            });
        let array_binding = self.resolve_array_binding_from_expression(&canonical_value_expression);
        let module_assignment_expression =
            self.materialize_static_expression(&canonical_value_expression);
        let resolved_local_binding = self.resolve_current_local_binding(name);
        if self.isolated_indirect_eval
            && resolved_local_binding.is_none()
            && self.parameter_scope_arguments_local_for(name).is_none()
        {
            if let Some(global_index) = self.module.global_bindings.get(name).copied() {
                self.module
                    .update_static_global_assignment_metadata(name, &module_assignment_expression);
                if exact_static_number.is_none()
                    && let Some(text) = static_string_value.clone()
                {
                    self.module
                        .global_value_bindings
                        .insert(name.to_string(), Expression::String(text));
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::String);
                }
                if let Some(function_binding) = function_binding.clone() {
                    self.module
                        .global_function_bindings
                        .insert(name.to_string(), function_binding);
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::Function);
                }
                self.preserve_exact_static_global_number_binding(
                    name,
                    &module_assignment_expression,
                );
                self.update_global_specialized_function_value(name, &module_assignment_expression)?;
                self.update_global_property_descriptor_value(name, &module_assignment_expression);
                self.push_local_get(value_local);
                self.push_global_set(global_index);
                return Ok(());
            }
            if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
                return Ok(());
            }
            if let Some(binding) = self.module.implicit_global_bindings.get(name).copied() {
                self.module
                    .update_static_global_assignment_metadata(name, &module_assignment_expression);
                if exact_static_number.is_none()
                    && let Some(text) = static_string_value.clone()
                {
                    self.module
                        .global_value_bindings
                        .insert(name.to_string(), Expression::String(text));
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::String);
                }
                if let Some(function_binding) = function_binding.clone() {
                    self.module
                        .global_function_bindings
                        .insert(name.to_string(), function_binding);
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::Function);
                }
                self.preserve_exact_static_global_number_binding(
                    name,
                    &module_assignment_expression,
                );
                self.update_global_specialized_function_value(name, &module_assignment_expression)?;
                self.ensure_global_property_descriptor_value(
                    name,
                    &module_assignment_expression,
                    true,
                );
                self.emit_store_implicit_global_from_local(binding, value_local)?;
                return Ok(());
            }
            let binding = self.module.ensure_implicit_global_binding(name);
            self.module
                .update_static_global_assignment_metadata(name, &module_assignment_expression);
            if exact_static_number.is_none()
                && let Some(text) = static_string_value.clone()
            {
                self.module
                    .global_value_bindings
                    .insert(name.to_string(), Expression::String(text));
                self.module
                    .global_kinds
                    .insert(name.to_string(), StaticValueKind::String);
            }
            if let Some(function_binding) = function_binding.clone() {
                self.module
                    .global_function_bindings
                    .insert(name.to_string(), function_binding);
                self.module
                    .global_kinds
                    .insert(name.to_string(), StaticValueKind::Function);
            }
            self.preserve_exact_static_global_number_binding(name, &module_assignment_expression);
            self.update_global_specialized_function_value(name, &module_assignment_expression)?;
            self.ensure_global_property_descriptor_value(name, &module_assignment_expression, true);
            self.emit_store_implicit_global_from_local(binding, value_local)?;
            return Ok(());
        }
        let resolved_name = resolved_local_binding
            .as_ref()
            .map(|(resolved_name, _)| resolved_name.as_str())
            .unwrap_or(name);
        let is_internal_array_iterator_binding = resolved_name.starts_with("__ayy_array_iter_");
        let is_internal_array_step_binding = resolved_name.starts_with("__ayy_array_step_");
        let is_internal_iterator_temp =
            is_internal_array_iterator_binding || is_internal_array_step_binding;
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(value_local);
            self.push_local_set(parameter_scope_arguments_local);
        }
        self.update_member_function_bindings_for_value(
            resolved_name,
            &canonical_value_expression,
            value_local,
        )?;
        if !is_internal_iterator_temp {
            self.update_local_function_binding(resolved_name, &canonical_value_expression);
            self.update_local_specialized_function_value(
                resolved_name,
                &canonical_value_expression,
            )?;
            self.update_local_proxy_binding(resolved_name, &canonical_value_expression);
            self.update_object_literal_member_bindings_for_value(
                resolved_name,
                &canonical_value_expression,
            );
            self.update_local_array_binding(resolved_name, &canonical_value_expression);
            self.update_local_resizable_array_buffer_binding(
                resolved_name,
                &canonical_value_expression,
            )?;
            self.update_local_typed_array_view_binding(resolved_name, &canonical_value_expression)?;
        }
        self.update_local_array_iterator_binding(resolved_name, &canonical_value_expression);
        self.update_local_iterator_step_binding(resolved_name, &canonical_value_expression);
        if is_internal_array_step_binding {
            self.local_kinds
                .insert(resolved_name.to_string(), StaticValueKind::Object);
        }
        if !is_internal_array_iterator_binding {
            self.update_local_object_binding(resolved_name, &canonical_value_expression);
        }
        if !is_internal_iterator_temp {
            self.update_local_arguments_binding(resolved_name, &canonical_value_expression);
            self.update_local_descriptor_binding(resolved_name, &canonical_value_expression);
        }

        if let Some((resolved_name, local_index)) = resolved_local_binding {
            if !is_internal_iterator_temp {
                self.update_local_value_binding(&resolved_name, &canonical_value_expression);
                self.local_kinds.insert(
                    resolved_name.clone(),
                    kind.unwrap_or(StaticValueKind::Unknown),
                );
            }
            self.push_local_get(value_local);
            self.push_local_set(local_index);
            if !is_internal_iterator_temp
                && let Some(source_name) = scoped_binding_source_name(name)
                && self
                    .resolve_eval_local_function_hidden_name(source_name)
                    .is_some()
            {
                self.update_local_value_binding(source_name, &canonical_value_expression);
                if let Some(function_binding) = function_binding.clone() {
                    self.local_function_bindings
                        .insert(source_name.to_string(), function_binding);
                } else {
                    self.local_function_bindings.remove(source_name);
                }
                self.local_kinds.insert(
                    source_name.to_string(),
                    kind.unwrap_or(StaticValueKind::Unknown),
                );
                self.emit_store_eval_local_function_binding_from_local(source_name, value_local)?;
            }
        } else if self
            .resolve_user_function_capture_hidden_name(name)
            .is_some()
        {
            if !is_internal_iterator_temp {
                self.update_local_value_binding(name, &canonical_value_expression);
                if let Some(function_binding) = function_binding.clone() {
                    self.local_function_bindings
                        .insert(name.to_string(), function_binding);
                } else {
                    self.local_function_bindings.remove(name);
                }
                self.local_kinds
                    .insert(name.to_string(), kind.unwrap_or(StaticValueKind::Unknown));
            }
            if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
                && !self.isolated_indirect_eval
            {
                self.module.update_static_global_assignment_metadata(
                    &hidden_name,
                    &module_assignment_expression,
                );
                self.preserve_exact_static_global_number_binding(
                    &hidden_name,
                    &module_assignment_expression,
                );
                self.update_global_specialized_function_value(
                    &hidden_name,
                    &module_assignment_expression,
                )?;
                self.ensure_global_property_descriptor_value(
                    &hidden_name,
                    &module_assignment_expression,
                    true,
                );
            }
            self.emit_store_user_function_capture_binding_from_local(name, value_local)?;
        } else if let Some(global_index) = self.module.global_bindings.get(name).copied() {
            if !self.isolated_indirect_eval {
                self.module
                    .update_static_global_assignment_metadata(name, &module_assignment_expression);
                if exact_static_number.is_none()
                    && let Some(text) = static_string_value.clone()
                {
                    self.module
                        .global_value_bindings
                        .insert(name.to_string(), Expression::String(text));
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::String);
                }
                if let Some(function_binding) = function_binding.clone() {
                    self.module
                        .global_function_bindings
                        .insert(name.to_string(), function_binding);
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::Function);
                }
                self.preserve_exact_static_global_number_binding(
                    name,
                    &module_assignment_expression,
                );
                self.update_global_specialized_function_value(name, &module_assignment_expression)?;
                self.update_global_property_descriptor_value(name, &module_assignment_expression);
            }
            self.push_local_get(value_local);
            self.push_global_set(global_index);
            if let Some(array_binding) = array_binding.as_ref() {
                self.emit_sync_global_runtime_array_state_from_binding(name, array_binding)?;
            }
        } else if self.resolve_eval_local_function_hidden_name(name).is_some() {
            self.update_local_value_binding(name, &canonical_value_expression);
            if let Some(function_binding) = function_binding {
                self.local_function_bindings
                    .insert(name.to_string(), function_binding);
            } else {
                self.local_function_bindings.remove(name);
            }
            self.local_kinds
                .insert(name.to_string(), kind.unwrap_or(StaticValueKind::Unknown));
            if let Some(source_name) = scoped_binding_source_name(name) {
                self.update_local_value_binding(source_name, &canonical_value_expression);
                if let Some(function_binding) = self.local_function_bindings.get(name).cloned() {
                    self.local_function_bindings
                        .insert(source_name.to_string(), function_binding);
                } else {
                    self.local_function_bindings.remove(source_name);
                }
                self.local_kinds.insert(
                    source_name.to_string(),
                    kind.unwrap_or(StaticValueKind::Unknown),
                );
            }
            self.emit_store_eval_local_function_binding_from_local(name, value_local)?;
        } else if let Some(binding) = self.module.implicit_global_bindings.get(name).copied() {
            if !self.isolated_indirect_eval {
                self.module
                    .update_static_global_assignment_metadata(name, &module_assignment_expression);
                if exact_static_number.is_none()
                    && let Some(text) = static_string_value.clone()
                {
                    self.module
                        .global_value_bindings
                        .insert(name.to_string(), Expression::String(text));
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::String);
                }
                if let Some(function_binding) = function_binding.clone() {
                    self.module
                        .global_function_bindings
                        .insert(name.to_string(), function_binding);
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::Function);
                }
                self.preserve_exact_static_global_number_binding(
                    name,
                    &module_assignment_expression,
                );
                self.update_global_specialized_function_value(name, &module_assignment_expression)?;
                self.ensure_global_property_descriptor_value(
                    name,
                    &module_assignment_expression,
                    true,
                );
            }
            self.emit_store_implicit_global_from_local(binding, value_local)?;
            if let Some(array_binding) = array_binding.as_ref() {
                self.emit_sync_global_runtime_array_state_from_binding(name, array_binding)?;
            }
        } else {
            let binding = self.module.ensure_implicit_global_binding(name);
            if !self.isolated_indirect_eval {
                self.module
                    .update_static_global_assignment_metadata(name, &module_assignment_expression);
                if exact_static_number.is_none()
                    && let Some(text) = static_string_value
                {
                    self.module
                        .global_value_bindings
                        .insert(name.to_string(), Expression::String(text));
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::String);
                }
                if let Some(function_binding) = function_binding {
                    self.module
                        .global_function_bindings
                        .insert(name.to_string(), function_binding);
                    self.module
                        .global_kinds
                        .insert(name.to_string(), StaticValueKind::Function);
                }
                self.preserve_exact_static_global_number_binding(
                    name,
                    &module_assignment_expression,
                );
                self.update_global_specialized_function_value(name, &module_assignment_expression)?;
                self.ensure_global_property_descriptor_value(
                    name,
                    &module_assignment_expression,
                    true,
                );
            }
            self.emit_store_implicit_global_from_local(binding, value_local)?;
            if let Some(array_binding) = array_binding.as_ref() {
                self.emit_sync_global_runtime_array_state_from_binding(name, array_binding)?;
            }
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn preserve_exact_static_global_number_binding(
        &mut self,
        name: &str,
        value_expression: &Expression,
    ) {
        let Some(number) = self.resolve_static_number_value(value_expression) else {
            return;
        };
        if number.is_nan() {
            return;
        }
        if number.is_finite()
            && number.fract() == 0.0
            && !(number == 0.0 && number.is_sign_negative())
        {
            return;
        }
        self.module
            .global_value_bindings
            .insert(name.to_string(), Expression::Number(number));
        self.module
            .global_kinds
            .insert(name.to_string(), StaticValueKind::Number);
    }

    pub(in crate::backend::direct_wasm) fn try_emit_destructuring_default_assign_statement(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<bool> {
        let Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } = value
        else {
            return Ok(false);
        };
        let Expression::Binary {
            op: BinaryOp::NotEqual,
            left,
            right,
        } = condition.as_ref()
        else {
            return Ok(false);
        };
        if !matches!(right.as_ref(), Expression::Undefined) {
            return Ok(false);
        }
        let Expression::Assign {
            name: temporary_name,
            value: temporary_value_expression,
        } = left.as_ref()
        else {
            return Ok(false);
        };
        let Expression::Identifier(then_name) = then_expression.as_ref() else {
            return Ok(false);
        };
        if then_name != temporary_name || !self.locals.contains_key(temporary_name) {
            return Ok(false);
        }
        let Expression::Member { object, property } = temporary_value_expression.as_ref() else {
            return Ok(false);
        };

        self.emit_numeric_expression(object)?;
        self.instructions.push(0x1a);
        let resolved_property = self.emit_property_key_expression_effects(property)?;
        let effective_property = resolved_property.as_ref().unwrap_or(property.as_ref());

        let scoped_target = self.resolve_with_scope_binding(name)?;

        self.emit_member_read_without_prelude(object, effective_property)?;
        let temporary_local = self.lookup_local(temporary_name)?;
        self.push_local_set(temporary_local);

        self.push_local_get(temporary_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(temporary_local);
        self.instructions.push(0x05);
        self.emit_numeric_expression(else_expression)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();

        let value_local = self.allocate_temp_local();
        self.push_local_set(value_local);
        if let Some(scope_object) = scoped_target {
            self.emit_scoped_property_store_from_local(&scope_object, name, value_local, value)?;
            self.instructions.push(0x1a);
        } else {
            self.emit_store_identifier_value_local(name, value, value_local)?;
        }

        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn note_identifier_numeric_kind(&mut self, name: &str) {
        let names = HashSet::from([name.to_string()]);
        let preserved_kinds = HashMap::from([(name.to_string(), StaticValueKind::Number)]);
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &names,
            &preserved_kinds,
        );
    }
}
