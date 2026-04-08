use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prepare_user_function_capture_bindings(
        &mut self,
        user_function: &UserFunction,
    ) -> DirectResult<Vec<PreparedCaptureBinding>> {
        let Some(capture_bindings) = self.user_function_capture_bindings(&user_function.name)
        else {
            return Ok(Vec::new());
        };

        let mut prepared = Vec::new();
        for (source_name, hidden_name) in capture_bindings {
            let binding = self
                .implicit_global_binding(&hidden_name)
                .unwrap_or_else(|| self.ensure_implicit_global_binding(&hidden_name));
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
        let Some(capture_bindings) = self.user_function_capture_bindings(function_name) else {
            return Ok(());
        };

        for (source_name, hidden_name) in capture_bindings {
            let binding = self
                .implicit_global_binding(&hidden_name)
                .unwrap_or_else(|| self.ensure_implicit_global_binding(&hidden_name));
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
            || (self.is_current_arguments_binding_name(name) && self.has_arguments_object())
            || self.resolve_current_local_binding(name).is_some()
            || self
                .state
                .speculation
                .static_semantics
                .has_local_function_binding(name)
            || (is_internal_user_function_identifier(name) && self.contains_user_function(name))
            || self.resolve_eval_local_function_hidden_name(name).is_some()
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some()
            || self.global_has_binding(name)
            || self.global_has_implicit_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn clear_user_function_capture_static_metadata(
        &mut self,
        hidden_name: &str,
    ) {
        self.backend
            .clear_global_static_binding_metadata(hidden_name);
    }

    pub(in crate::backend::direct_wasm) fn sync_user_function_capture_static_metadata(
        &mut self,
        source_name: &str,
        hidden_name: &str,
    ) {
        let source_expression = Expression::Identifier(source_name.to_string());
        let inferred_kind = self.infer_value_kind(&source_expression);
        let resolved_value = self.resolve_bound_alias_expression(&source_expression);

        self.backend.sync_global_expression_binding(
            hidden_name,
            resolved_value.filter(|value| !static_expression_matches(value, &source_expression)),
        );
        self.backend.sync_global_array_binding(
            hidden_name,
            self.resolve_array_binding_from_expression(&source_expression),
        );
        self.backend.sync_global_object_binding(
            hidden_name,
            self.resolve_object_binding_from_expression(&source_expression),
        );
        self.backend.sync_global_function_binding(
            hidden_name,
            self.resolve_function_binding_from_expression(&source_expression),
        );

        if let Some(kind) = inferred_kind {
            self.backend.set_global_binding_kind(hidden_name, kind);
        } else {
            self.clear_global_binding_kind(hidden_name);
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
                    .backend
                    .global_semantics
                    .names
                    .bindings
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
                self.state
                    .runtime
                    .locals
                    .runtime_dynamic_bindings
                    .insert(binding.source_name.clone());
            } else {
                self.state
                    .runtime
                    .locals
                    .runtime_dynamic_bindings
                    .remove(&binding.source_name);
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
                    .backend
                    .global_semantics
                    .names
                    .kinds
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
            && self
                .backend
                .global_semantics
                .names
                .bindings
                .contains_key(source_name)
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
            if self
                .backend
                .global_semantics
                .names
                .bindings
                .contains_key(source_name)
                || self.global_has_implicit_binding(source_name)
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
}
