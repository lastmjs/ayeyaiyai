use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prepare_user_function_capture_bindings(
        &mut self,
        user_function: &UserFunction,
    ) -> DirectResult<Vec<PreparedCaptureBinding>> {
        let Some(capture_bindings) = self
            .module
            .user_function_capture_bindings
            .get(&user_function.name)
            .cloned()
        else {
            return Ok(Vec::new());
        };

        let mut prepared = Vec::new();
        for (source_name, hidden_name) in capture_bindings {
            let binding = self
                .module
                .implicit_global_bindings
                .get(&hidden_name)
                .copied()
                .unwrap_or_else(|| self.module.ensure_implicit_global_binding(&hidden_name));
            let saved_value_local = self.allocate_temp_local();
            let saved_present_local = self.allocate_temp_local();
            self.push_global_get(binding.value_index);
            self.push_local_set(saved_value_local);
            self.push_global_get(binding.present_index);
            self.push_local_set(saved_present_local);
            prepared.push(PreparedCaptureBinding {
                binding,
                source_name,
                hidden_name,
                saved_value_local,
                saved_present_local,
            });
        }

        Ok(prepared)
    }

    pub(in crate::backend::direct_wasm) fn emit_prepare_user_function_capture_globals(
        &mut self,
        function_name: &str,
    ) -> DirectResult<()> {
        let Some(capture_bindings) = self
            .module
            .user_function_capture_bindings
            .get(function_name)
            .cloned()
        else {
            return Ok(());
        };

        for (source_name, hidden_name) in capture_bindings {
            let binding = self
                .module
                .implicit_global_bindings
                .get(&hidden_name)
                .copied()
                .unwrap_or_else(|| self.module.ensure_implicit_global_binding(&hidden_name));
            if !self.user_function_capture_source_is_locally_bound(&source_name) {
                self.clear_user_function_capture_static_metadata(&hidden_name);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_global_set(binding.value_index);
                self.push_i32_const(0);
                self.push_global_set(binding.present_index);
                continue;
            }
            self.sync_user_function_capture_static_metadata(&source_name, &hidden_name);
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(&Expression::Identifier(source_name.clone()))?;
            self.push_local_set(value_local);
            self.push_local_get(value_local);
            self.push_global_set(binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(binding.present_index);
            self.emit_runtime_object_property_shadow_copy(&source_name, &hidden_name)?;
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_source_is_locally_bound(
        &self,
        name: &str,
    ) -> bool {
        self.parameter_scope_arguments_local_for(name).is_some()
            || (name == "arguments" && self.has_arguments_object())
            || self.resolve_current_local_binding(name).is_some()
            || self.local_function_bindings.contains_key(name)
            || (is_internal_user_function_identifier(name)
                && self.module.user_function_map.contains_key(name))
            || self.resolve_eval_local_function_hidden_name(name).is_some()
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some()
            || self.module.global_bindings.contains_key(name)
            || self.module.implicit_global_bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn clear_user_function_capture_static_metadata(
        &mut self,
        hidden_name: &str,
    ) {
        self.module.global_value_bindings.remove(hidden_name);
        self.module.global_array_bindings.remove(hidden_name);
        self.module.global_object_bindings.remove(hidden_name);
        self.module.global_function_bindings.remove(hidden_name);
        self.module.global_kinds.remove(hidden_name);
    }

    pub(in crate::backend::direct_wasm) fn sync_user_function_capture_static_metadata(
        &mut self,
        source_name: &str,
        hidden_name: &str,
    ) {
        let source_expression = Expression::Identifier(source_name.to_string());
        let inferred_kind = self.infer_value_kind(&source_expression);
        let resolved_value = self.resolve_bound_alias_expression(&source_expression);

        if let Some(value) =
            resolved_value.filter(|value| !static_expression_matches(value, &source_expression))
        {
            self.module
                .global_value_bindings
                .insert(hidden_name.to_string(), value);
        } else {
            self.module.global_value_bindings.remove(hidden_name);
        }

        if let Some(array_binding) = self.resolve_array_binding_from_expression(&source_expression)
        {
            self.module
                .global_array_bindings
                .insert(hidden_name.to_string(), array_binding);
        } else {
            self.module.global_array_bindings.remove(hidden_name);
        }

        if let Some(object_binding) =
            self.resolve_object_binding_from_expression(&source_expression)
        {
            self.module
                .global_object_bindings
                .insert(hidden_name.to_string(), object_binding);
        } else {
            self.module.global_object_bindings.remove(hidden_name);
        }

        if let Some(function_binding) =
            self.resolve_function_binding_from_expression(&source_expression)
        {
            self.module
                .global_function_bindings
                .insert(hidden_name.to_string(), function_binding);
        } else {
            self.module.global_function_bindings.remove(hidden_name);
        }

        if let Some(kind) = inferred_kind {
            self.module
                .global_kinds
                .insert(hidden_name.to_string(), kind);
        } else {
            self.module.global_kinds.remove(hidden_name);
        }
    }

    pub(in crate::backend::direct_wasm) fn restore_user_function_capture_bindings(
        &mut self,
        prepared: &[PreparedCaptureBinding],
    ) {
        for binding in prepared.iter().rev() {
            self.push_local_get(binding.saved_value_local);
            self.push_global_set(binding.binding.value_index);
            self.push_local_get(binding.saved_present_local);
            self.push_global_set(binding.binding.present_index);
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_user_function_capture_source_bindings(
        &mut self,
        prepared: &[PreparedCaptureBinding],
        assigned_nonlocal_bindings: &HashSet<String>,
        call_effect_nonlocal_bindings: &HashSet<String>,
        updated_nonlocal_bindings: &HashSet<String>,
        updated_bindings: Option<&HashMap<String, Expression>>,
    ) -> DirectResult<()> {
        for binding in prepared {
            if !self.user_function_capture_source_is_locally_bound(&binding.source_name) {
                continue;
            }
            let skip_runtime_source_sync = updated_nonlocal_bindings.contains(&binding.source_name)
                && self
                    .module
                    .global_bindings
                    .contains_key(&binding.source_name);
            let value_local = self.allocate_temp_local();
            self.push_global_get(binding.binding.value_index);
            self.push_local_set(value_local);
            let source_is_dynamic = self.sync_user_function_capture_source_static_metadata(
                &binding.source_name,
                &binding.hidden_name,
                assigned_nonlocal_bindings,
                call_effect_nonlocal_bindings,
                updated_nonlocal_bindings,
                updated_bindings,
            )?;
            if source_is_dynamic {
                self.runtime_dynamic_bindings
                    .insert(binding.source_name.clone());
            } else {
                self.runtime_dynamic_bindings.remove(&binding.source_name);
            }
            if !skip_runtime_source_sync {
                self.emit_sync_identifier_runtime_value_from_local(
                    &binding.source_name,
                    value_local,
                )?;
                self.emit_runtime_object_property_shadow_copy(
                    &binding.hidden_name,
                    &binding.source_name,
                )?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn sync_user_function_capture_source_static_metadata(
        &mut self,
        source_name: &str,
        hidden_name: &str,
        assigned_nonlocal_bindings: &HashSet<String>,
        call_effect_nonlocal_bindings: &HashSet<String>,
        updated_nonlocal_bindings: &HashSet<String>,
        updated_bindings: Option<&HashMap<String, Expression>>,
    ) -> DirectResult<bool> {
        let invalidate_source = |compiler: &mut Self, preserve_kind: bool| {
            let names = HashSet::from([source_name.to_string()]);
            if preserve_kind {
                if let Some(kind) = compiler
                    .module
                    .global_kinds
                    .get(hidden_name)
                    .copied()
                    .or_else(|| compiler.lookup_identifier_kind(source_name))
                {
                    let preserved_kinds = HashMap::from([(source_name.to_string(), kind)]);
                    compiler.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                        &names,
                        &preserved_kinds,
                    );
                    return;
                }
            }
            compiler.invalidate_static_binding_metadata_for_names(&names);
        };

        if (updated_nonlocal_bindings.contains(source_name)
            || (!assigned_nonlocal_bindings.contains(source_name)
                && call_effect_nonlocal_bindings.contains(source_name)
                && updated_bindings
                    .and_then(|bindings| bindings.get(source_name))
                    .is_none()))
            && self.module.global_bindings.contains_key(source_name)
        {
            invalidate_source(self, true);
            return Ok(true);
        }

        let hidden_expression = Expression::Identifier(hidden_name.to_string());
        let resolved_hidden_value = self.resolve_bound_alias_expression(&hidden_expression);
        if assigned_nonlocal_bindings.contains(source_name) {
            if let Some(value) = updated_bindings.and_then(|bindings| bindings.get(source_name)) {
                self.sync_bound_capture_source_binding_metadata(source_name, value)?;
                return Ok(false);
            }
            if self.module.global_bindings.contains_key(source_name)
                || self
                    .module
                    .implicit_global_bindings
                    .contains_key(source_name)
            {
                invalidate_source(self, true);
                return Ok(true);
            }
            match resolved_hidden_value {
                Some(Expression::Identifier(name)) if name == hidden_name => {
                    invalidate_source(self, true);
                    return Ok(true);
                }
                Some(value) => {
                    self.sync_bound_capture_source_binding_metadata(source_name, &value)?;
                    return Ok(false);
                }
                None => {
                    invalidate_source(self, false);
                    return Ok(true);
                }
            }
        }

        match resolved_hidden_value {
            Some(Expression::Identifier(name)) if name == hidden_name => {
                invalidate_source(self, true);
                Ok(true)
            }
            Some(value) => {
                self.sync_bound_capture_source_binding_metadata(source_name, &value)?;
                Ok(false)
            }
            None => {
                invalidate_source(self, false);
                Ok(true)
            }
        }
    }

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
        let Some(capture_bindings) = self
            .module
            .user_function_capture_bindings
            .get(&user_function.name)
            .cloned()
        else {
            return Ok(Vec::new());
        };

        let mut prepared = Vec::new();
        for (capture_name, capture_hidden_name) in capture_bindings {
            let Some(slot_name) = capture_slots.get(&capture_name) else {
                continue;
            };
            let Some(slot_local) = self.locals.get(slot_name).copied() else {
                continue;
            };
            let source_binding_name = self
                .capture_slot_source_bindings
                .get(slot_name)
                .cloned()
                .or_else(|| {
                    self.local_value_bindings.get(slot_name).and_then(|value| {
                        let Expression::Identifier(name) =
                            self.materialize_static_expression(value)
                        else {
                            return None;
                        };
                        Some(name)
                    })
                });
            let binding = self
                .module
                .implicit_global_bindings
                .get(&capture_hidden_name)
                .copied()
                .unwrap_or_else(|| {
                    self.module
                        .ensure_implicit_global_binding(&capture_hidden_name)
                });
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
            self.local_value_bindings.remove(name);
            self.local_array_bindings.remove(name);
            self.local_object_bindings.remove(name);
            self.local_function_bindings.remove(name);
            self.local_kinds.remove(name);
            self.local_arguments_bindings.remove(name);
            self.local_descriptor_bindings.remove(name);
            self.local_proxy_bindings.remove(name);
            self.local_prototype_object_bindings.remove(name);
            self.local_specialized_function_values.remove(name);
        }

        if !is_local_binding
            && (self.module.global_bindings.contains_key(name)
                || self.module.implicit_global_bindings.contains_key(name))
        {
            self.module
                .update_static_global_assignment_metadata(name, value);
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

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
        this_expression: &Expression,
        capture_slots: &BTreeMap<String, String>,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let prepared_capture_bindings =
            self.prepare_bound_user_function_capture_bindings(user_function, capture_slots)?;
        let synced_capture_source_bindings = self
            .synced_prepared_bound_user_function_capture_source_bindings(
                &prepared_capture_bindings,
            );

        let saved_new_target_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(new_target_value);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            Some(saved_local)
        };
        let saved_this_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            let this_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.emit_numeric_expression(this_expression)?;
            self.push_local_set(this_local);
            self.push_local_get(this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
            Some(saved_local)
        };

        let capture_snapshot = capture_slots
            .iter()
            .map(|(capture_name, slot_name)| {
                (
                    capture_name.clone(),
                    self.snapshot_bound_capture_slot_expression(slot_name),
                )
            })
            .collect::<HashMap<_, _>>();
        let static_result = self
            .resolve_bound_snapshot_user_function_result(&user_function.name, &capture_snapshot);
        self.last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
            function_name: user_function.name.clone(),
            source_expression: None,
            result_expression: static_result.as_ref().map(|(result, _)| result.clone()),
            updated_bindings: static_result
                .as_ref()
                .map(|(_, updated_bindings)| updated_bindings.clone())
                .unwrap_or_else(|| capture_snapshot.clone()),
        });

        self.emit_prepare_bound_user_function_capture_globals(&prepared_capture_bindings)?;

        let visible_param_count = user_function.visible_param_count() as usize;
        let tracked_extra_indices = user_function
            .extra_argument_indices
            .iter()
            .map(|index| *index as usize)
            .collect::<HashSet<_>>();
        let mut argument_locals = HashMap::new();

        for (argument_index, argument) in expanded_arguments.iter().enumerate() {
            if argument_index < visible_param_count
                || tracked_extra_indices.contains(&argument_index)
            {
                let argument_local = self.allocate_temp_local();
                self.emit_numeric_expression(argument)?;
                self.push_local_set(argument_local);
                argument_locals.insert(argument_index, argument_local);
            } else {
                self.emit_numeric_expression(argument)?;
                self.instructions.push(0x1a);
            }
        }

        for argument_index in 0..visible_param_count {
            if let Some(argument_local) = argument_locals.get(&argument_index).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        self.push_i32_const(expanded_arguments.len() as i32);

        for index in &user_function.extra_argument_indices {
            if let Some(argument_local) = argument_locals.get(&(*index as usize)).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        self.push_call(user_function.function_index);
        let return_value_local = self.allocate_temp_local();
        self.push_local_set(return_value_local);
        let updated_bindings = self
            .last_bound_user_function_call
            .as_ref()
            .and_then(|snapshot| {
                (snapshot.function_name == user_function.name)
                    .then_some(snapshot.updated_bindings.clone())
            });
        self.sync_bound_user_function_capture_slots(
            &prepared_capture_bindings,
            updated_bindings.as_ref(),
        )?;
        self.restore_bound_user_function_capture_bindings(&prepared_capture_bindings);
        self.invalidate_user_function_call_effect_nonlocal_bindings_except(
            user_function,
            &synced_capture_source_bindings,
        );
        if let Some(saved_new_target_local) = saved_new_target_local {
            self.push_local_get(saved_new_target_local);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
        }
        if let Some(saved_this_local) = saved_this_local {
            self.push_local_get(saved_this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
        }
        if user_function.is_async() {
            self.push_global_get(THROW_TAG_GLOBAL_INDEX);
            self.push_i32_const(0);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.clear_global_throw_state();
            self.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        self.emit_check_global_throw_for_user_call()?;
        self.push_local_get(return_value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_from_argument_locals(
        &mut self,
        user_function: &UserFunction,
        argument_locals: &[u32],
        argument_count: usize,
        new_target_value: i32,
        this_expression: &Expression,
        capture_slots: &BTreeMap<String, String>,
    ) -> DirectResult<()> {
        let prepared_capture_bindings =
            self.prepare_bound_user_function_capture_bindings(user_function, capture_slots)?;
        let synced_capture_source_bindings = self
            .synced_prepared_bound_user_function_capture_source_bindings(
                &prepared_capture_bindings,
            );

        let saved_new_target_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(new_target_value);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            Some(saved_local)
        };
        let saved_this_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            let this_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.emit_numeric_expression(this_expression)?;
            self.push_local_set(this_local);
            self.push_local_get(this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
            Some(saved_local)
        };

        let capture_snapshot = capture_slots
            .iter()
            .map(|(capture_name, slot_name)| {
                (
                    capture_name.clone(),
                    self.snapshot_bound_capture_slot_expression(slot_name),
                )
            })
            .collect::<HashMap<_, _>>();
        let static_result = self
            .resolve_bound_snapshot_user_function_result(&user_function.name, &capture_snapshot);
        self.last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
            function_name: user_function.name.clone(),
            source_expression: None,
            result_expression: static_result.as_ref().map(|(result, _)| result.clone()),
            updated_bindings: static_result
                .as_ref()
                .map(|(_, updated_bindings)| updated_bindings.clone())
                .unwrap_or_else(|| capture_snapshot.clone()),
        });

        self.emit_prepare_bound_user_function_capture_globals(&prepared_capture_bindings)?;

        let visible_param_count = user_function.visible_param_count() as usize;

        for argument_index in 0..visible_param_count {
            if let Some(argument_local) = argument_locals.get(argument_index).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        self.push_i32_const(argument_count as i32);

        for index in &user_function.extra_argument_indices {
            if let Some(argument_local) = argument_locals.get(*index as usize).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        self.push_call(user_function.function_index);
        let return_value_local = self.allocate_temp_local();
        self.push_local_set(return_value_local);
        let updated_bindings = self
            .last_bound_user_function_call
            .as_ref()
            .and_then(|snapshot| {
                (snapshot.function_name == user_function.name)
                    .then_some(snapshot.updated_bindings.clone())
            });
        self.sync_bound_user_function_capture_slots(
            &prepared_capture_bindings,
            updated_bindings.as_ref(),
        )?;
        self.restore_bound_user_function_capture_bindings(&prepared_capture_bindings);
        self.invalidate_user_function_call_effect_nonlocal_bindings_except(
            user_function,
            &synced_capture_source_bindings,
        );
        if let Some(saved_new_target_local) = saved_new_target_local {
            self.push_local_get(saved_new_target_local);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
        }
        if let Some(saved_this_local) = saved_this_local {
            self.push_local_get(saved_this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
        }
        if user_function.is_async() {
            self.push_global_get(THROW_TAG_GLOBAL_INDEX);
            self.push_i32_const(0);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.clear_global_throw_state();
            self.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        self.emit_check_global_throw_for_user_call()?;
        self.push_local_get(return_value_local);
        Ok(())
    }
}
