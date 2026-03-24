use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_eval_local_function_hidden_name(
        &self,
        name: &str,
    ) -> Option<String> {
        let current_function_name = self.current_user_function_name.as_deref()?;
        let bindings = self
            .module
            .eval_local_function_bindings
            .get(current_function_name)?;
        if let Some(hidden_name) = bindings.get(name) {
            return Some(hidden_name.clone());
        }

        let renamed_prefix = format!("__ayy_scope${name}$");
        let mut resolved: Option<(u32, String)> = None;
        for (candidate_name, hidden_name) in bindings {
            if !candidate_name.starts_with(&renamed_prefix) {
                continue;
            }
            let Some((_, scope_id)) = candidate_name.rsplit_once('$') else {
                continue;
            };
            let Ok(scope_id) = scope_id.parse::<u32>() else {
                continue;
            };
            if resolved
                .as_ref()
                .is_none_or(|(best_scope_id, _)| scope_id > *best_scope_id)
            {
                resolved = Some((scope_id, hidden_name.clone()));
            }
        }

        resolved.map(|(_, hidden_name)| hidden_name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_capture_hidden_name(
        &self,
        name: &str,
    ) -> Option<String> {
        let current_function_name = self.current_user_function_name.as_deref()?;
        let bindings = self
            .module
            .user_function_capture_bindings
            .get(current_function_name)?;
        if let Some(hidden_name) = bindings.get(name) {
            return Some(hidden_name.clone());
        }

        let source_name = scoped_binding_source_name(name);
        if let Some(source_name) = source_name
            && let Some(hidden_name) = bindings.get(source_name)
        {
            return Some(hidden_name.clone());
        }

        bindings.iter().find_map(|(capture_name, hidden_name)| {
            self.resolve_registered_function_declaration(capture_name)
                .and_then(|function| function.self_binding.as_deref())
                .filter(|self_binding| {
                    *self_binding == name
                        || source_name.is_some_and(|source_name| *self_binding == source_name)
                })
                .map(|_| hidden_name.clone())
        })
    }

    pub(in crate::backend::direct_wasm) fn emit_eval_local_function_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };

        self.push_global_get(binding.present_index);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.instructions.push(0x05);
        self.emit_named_error_throw("ReferenceError")?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_capture_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };

        self.push_global_get(binding.present_index);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.instructions.push(0x05);
        self.emit_named_error_throw("ReferenceError")?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_store_user_function_capture_binding_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };
        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_store_eval_local_function_binding_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };
        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_delete_eval_local_function_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_eval_local_function_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };
        let value_local = self.allocate_temp_local();

        self.push_global_get(binding.present_index);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.push_local_set(value_local);
        self.emit_runtime_typeof_tag_from_local(value_local)?;
        self.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_UNDEFINED_TAG);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_user_function_capture_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self
            .module
            .implicit_global_bindings
            .get(&hidden_name)
            .copied()
        else {
            return Ok(false);
        };
        let value_local = self.allocate_temp_local();

        self.push_global_get(binding.present_index);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.push_local_set(value_local);
        self.emit_runtime_typeof_tag_from_local(value_local)?;
        self.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_UNDEFINED_TAG);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn clear_eval_local_function_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.local_value_bindings.remove(name);
        self.local_function_bindings.remove(name);
        self.local_kinds.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_static_identifier_binding_metadata(
        &mut self,
        name: &str,
    ) {
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

        self.module.global_value_bindings.remove(name);
        self.module.global_array_bindings.remove(name);
        self.module.global_object_bindings.remove(name);
        self.module.global_function_bindings.remove(name);
        self.module.global_kinds.remove(name);
        self.module.global_arguments_bindings.remove(name);
        self.module.global_proxy_bindings.remove(name);
        self.module.global_prototype_object_bindings.remove(name);
        self.module.global_property_descriptors.remove(name);
        self.module.global_specialized_function_values.remove(name);
        self.module
            .clear_global_object_literal_member_bindings_for_name(name);
    }

    pub(in crate::backend::direct_wasm) fn emit_delete_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(binding) = self.module.implicit_global_bindings.get(name).copied() else {
            return Ok(false);
        };
        self.clear_static_identifier_binding_metadata(name);
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_object_property_shadow_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> bool {
        let Some(binding) = self.resolve_runtime_object_property_shadow_binding(object, property)
        else {
            return false;
        };
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        true
    }

    pub(in crate::backend::direct_wasm) fn canonicalize_with_scope_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        match expression {
            Expression::Identifier(_) => expression.clone(),
            Expression::Assign { name, .. } => Expression::Identifier(name.clone()),
            _ => self.materialize_static_expression(expression),
        }
    }

    pub(in crate::backend::direct_wasm) fn scope_object_has_binding_property(
        &self,
        scope_object: &Expression,
        name: &str,
    ) -> bool {
        let property = Expression::String(name.to_string());
        self.resolve_member_function_binding(scope_object, &property)
            .is_some()
            || self
                .resolve_member_getter_binding(scope_object, &property)
                .is_some()
            || self
                .resolve_member_setter_binding(scope_object, &property)
                .is_some()
            || self
                .resolve_object_binding_from_expression(scope_object)
                .is_some_and(|object_binding| {
                    object_binding_has_property(&object_binding, &property)
                })
    }

    pub(in crate::backend::direct_wasm) fn emit_with_scope_unscopables_block_check(
        &mut self,
        scope_object: &Expression,
        name: &str,
    ) -> DirectResult<bool> {
        let unscopables_key = Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("unscopables".to_string())),
        };
        let property = Expression::String(name.to_string());

        if let Some(getter_binding) =
            self.resolve_member_getter_binding(scope_object, &unscopables_key)
        {
            let Some(unscopables_object) =
                self.resolve_function_binding_static_return_object_binding(&getter_binding, &[])
            else {
                return Err(Unsupported("dynamic with @@unscopables getter"));
            };
            let blocked = object_binding_lookup_value(&unscopables_object, &property)
                .and_then(|value| self.resolve_static_boolean_expression(value))
                .unwrap_or(false);
            self.emit_function_binding_side_effects_with_arguments(&getter_binding, &[])?;
            return Ok(blocked);
        }

        let Some(scope_binding) = self.resolve_object_binding_from_expression(scope_object) else {
            return Ok(false);
        };
        let Some(unscopables_value) =
            object_binding_lookup_value(&scope_binding, &unscopables_key).cloned()
        else {
            return Ok(false);
        };
        let Some(unscopables_object) =
            self.resolve_object_binding_from_expression(&unscopables_value)
        else {
            return Ok(false);
        };
        Ok(object_binding_lookup_value(&unscopables_object, &property)
            .and_then(|value| self.resolve_static_boolean_expression(value))
            .unwrap_or(false))
    }

    pub(in crate::backend::direct_wasm) fn resolve_with_scope_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<Option<Expression>> {
        let scopes = self.with_scopes.clone();
        let property = Expression::String(name.to_string());
        for scope_object in scopes.into_iter().rev() {
            if let Some(proxy_binding) = self.resolve_proxy_binding_from_expression(&scope_object) {
                if let Some(has_binding) = proxy_binding.has_binding.clone() {
                    let arguments = [proxy_binding.target.clone(), property.clone()];
                    let Some(has_binding_result) =
                        self.resolve_function_binding_static_return_bool(&has_binding, &arguments)
                    else {
                        return Err(Unsupported("dynamic with proxy has"));
                    };
                    self.emit_function_binding_side_effects_with_arguments(
                        &has_binding,
                        &arguments,
                    )?;
                    if has_binding_result {
                        return Ok(Some(proxy_binding.target));
                    }
                    continue;
                }
                if self.scope_object_has_binding_property(&proxy_binding.target, name)
                    && !self.emit_with_scope_unscopables_block_check(&proxy_binding.target, name)?
                {
                    return Ok(Some(proxy_binding.target));
                }
                continue;
            }

            if !self.scope_object_has_binding_property(&scope_object, name) {
                continue;
            }
            if self.emit_with_scope_unscopables_block_check(&scope_object, name)? {
                continue;
            }
            return Ok(Some(scope_object));
        }

        Ok(None)
    }

    pub(in crate::backend::direct_wasm) fn resolve_current_local_binding(
        &self,
        name: &str,
    ) -> Option<(String, u32)> {
        fn resolve_current_local_binding_exact(
            locals: &HashMap<String, u32>,
            active_scoped_lexical_bindings: &HashMap<String, Vec<String>>,
            name: &str,
        ) -> Option<(String, u32)> {
            if let Some(local_index) = locals.get(name).copied() {
                return Some((name.to_string(), local_index));
            }

            if let Some(active_name) = active_scoped_lexical_bindings
                .get(name)
                .and_then(|bindings| bindings.last())
                .cloned()
            {
                if let Some(local_index) = locals.get(&active_name).copied() {
                    return Some((active_name, local_index));
                }
            }

            let mut scoped_matches = locals.iter().filter_map(|(binding_name, &local_index)| {
                (scoped_binding_source_name(binding_name) == Some(name))
                    .then(|| (binding_name.clone(), local_index))
            });
            let scoped_match = scoped_matches.next()?;
            scoped_matches.next().is_none().then_some(scoped_match)
        }

        if let Some(resolved) = resolve_current_local_binding_exact(
            &self.locals,
            &self.active_scoped_lexical_bindings,
            name,
        ) {
            return Some(resolved);
        }
        if let Some(source_name) = scoped_binding_source_name(name) {
            return resolve_current_local_binding_exact(
                &self.locals,
                &self.active_scoped_lexical_bindings,
                source_name,
            );
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn emit_eval_lexical_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(initialized_local) = self.eval_lexical_initialized_locals.get(name).copied()
        else {
            return Ok(false);
        };
        let local_index = self
            .locals
            .get(name)
            .copied()
            .expect("tracked eval lexical binding must have a local slot");
        self.push_local_get(initialized_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(local_index);
        self.instructions.push(0x05);
        self.emit_named_error_throw("ReferenceError")?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_plain_identifier_read_fallback(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if self.emit_eval_lexical_binding_read(name)? {
            return Ok(());
        }
        if self.emit_parameter_default_binding_read(name)? {
            return Ok(());
        }
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(parameter_scope_arguments_local);
        } else if parse_test262_realm_identifier(name).is_some()
            || parse_test262_realm_global_identifier(name).is_some()
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        } else if name == "arguments" && self.has_arguments_object() {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        } else if let Some((_, local_index)) = self.resolve_current_local_binding(name) {
            self.push_local_get(local_index);
        } else if let Some(function_binding) = self.local_function_bindings.get(name).cloned() {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(runtime_value) = self
                        .module
                        .user_function_map
                        .get(&function_name)
                        .map(user_function_runtime_value)
                    {
                        self.emit_prepare_user_function_capture_globals(&function_name)?;
                        self.push_i32_const(runtime_value);
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    self.push_i32_const(
                        builtin_function_runtime_value(&function_name)
                            .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                    );
                }
            }
        } else if let Some(global_index) = self.module.global_bindings.get(name).copied() {
            self.push_global_get(global_index);
        } else if self.emit_user_function_capture_binding_read(name)? {
        } else if self.emit_eval_local_function_binding_read(name)? {
        } else if name == "NaN" && self.is_unshadowed_builtin_identifier(name) {
            self.push_i32_const(JS_NAN_TAG);
        } else if name == "undefined" {
            self.push_i32_const(JS_UNDEFINED_TAG);
        } else if let Some(runtime_value) = builtin_function_runtime_value(name) {
            self.push_i32_const(runtime_value);
        } else if is_internal_user_function_identifier(name)
            && let Some(runtime_value) = self
                .module
                .user_function_map
                .get(name)
                .map(user_function_runtime_value)
        {
            self.emit_prepare_user_function_capture_globals(name)?;
            self.push_i32_const(runtime_value);
        } else if let Some(kind) = self.lookup_identifier_kind(name) {
            let tag = kind.as_typeof_tag().unwrap_or(JS_UNDEFINED_TAG);
            self.push_i32_const(tag);
        } else {
            self.emit_named_error_throw("ReferenceError")?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_plain_identifier_read(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if self.parameter_scope_arguments_local_for(name).is_some()
            || (name == "arguments" && self.has_arguments_object())
            || self.resolve_current_local_binding(name).is_some()
            || self.local_function_bindings.contains_key(name)
            || self.module.global_bindings.contains_key(name)
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some()
            || self.resolve_eval_local_function_hidden_name(name).is_some()
        {
            return self.emit_plain_identifier_read_fallback(name);
        }

        let Some(binding) = self.module.implicit_global_bindings.get(name).copied() else {
            return self.emit_plain_identifier_read_fallback(name);
        };

        self.push_global_get(binding.present_index);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.instructions.push(0x05);
        self.emit_plain_identifier_read_fallback(name)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_store_implicit_global_from_local(
        &mut self,
        binding: ImplicitGlobalBinding,
        value_local: u32,
    ) -> DirectResult<()> {
        if self.strict_mode {
            self.push_global_get(binding.present_index);
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_local_get(value_local);
            self.push_global_set(binding.value_index);
            self.instructions.push(0x05);
            self.emit_named_error_throw("ReferenceError")?;
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        }

        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn runtime_object_property_shadow_binding_name(
        owner_name: &str,
        property_name: &str,
    ) -> String {
        format!("__ayy_object_property__{owner_name}__{property_name}")
    }

    pub(in crate::backend::direct_wasm) fn runtime_object_property_shadow_owner_name_for_identifier(
        &self,
        name: &str,
    ) -> Option<String> {
        if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) {
            return Some(hidden_name);
        }
        if let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) {
            return Some(hidden_name);
        }
        if self.local_object_bindings.contains_key(name) {
            return Some(name.to_string());
        }
        ((self.module.global_bindings.contains_key(name)
            || self.module.implicit_global_bindings.contains_key(name))
            && self.module.global_object_bindings.contains_key(name))
        .then(|| name.to_string())
        .or_else(|| {
            (self.module.implicit_global_bindings.contains_key(name)
                && self.module.global_object_bindings.contains_key(name))
            .then(|| name.to_string())
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_runtime_object_property_shadow_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> Option<ImplicitGlobalBinding> {
        let Expression::Identifier(name) = object else {
            return None;
        };
        let property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let Expression::String(property_name) = property else {
            return None;
        };
        let owner_name = self.runtime_object_property_shadow_owner_name_for_identifier(name)?;
        Some(self.module.ensure_implicit_global_binding(
            &Self::runtime_object_property_shadow_binding_name(&owner_name, &property_name),
        ))
    }

    pub(in crate::backend::direct_wasm) fn runtime_object_property_shadow_binding_by_names(
        &mut self,
        owner_name: &str,
        property_name: &str,
    ) -> ImplicitGlobalBinding {
        self.module.ensure_implicit_global_binding(
            &Self::runtime_object_property_shadow_binding_name(owner_name, property_name),
        )
    }

    pub(in crate::backend::direct_wasm) fn object_runtime_shadow_properties(
        &self,
        owner_name: &str,
    ) -> Vec<(String, Expression)> {
        let object_expression = Expression::Identifier(owner_name.to_string());
        let Some(object_binding) = self.resolve_object_binding_from_expression(&object_expression)
        else {
            return Vec::new();
        };
        ordered_object_property_names(&object_binding)
            .into_iter()
            .filter_map(|property_name| {
                object_binding_lookup_value(
                    &object_binding,
                    &Expression::String(property_name.clone()),
                )
                .cloned()
                .map(|value| (property_name, value))
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_object_property_shadow_copy(
        &mut self,
        source_owner: &str,
        target_owner: &str,
    ) -> DirectResult<()> {
        if source_owner == target_owner {
            return Ok(());
        }
        for (property_name, fallback_value) in self.object_runtime_shadow_properties(source_owner) {
            let source_binding =
                self.runtime_object_property_shadow_binding_by_names(source_owner, &property_name);
            let target_binding =
                self.runtime_object_property_shadow_binding_by_names(target_owner, &property_name);
            self.push_global_get(source_binding.present_index);
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_global_get(source_binding.value_index);
            self.push_global_set(target_binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(target_binding.present_index);
            self.instructions.push(0x05);
            if let Some(function_binding) =
                self.resolve_function_binding_from_expression(&fallback_value)
            {
                match function_binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) =
                            self.module.user_function_map.get(&function_name)
                        {
                            self.push_i32_const(user_function_runtime_value(user_function));
                        } else {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        self.push_i32_const(
                            builtin_function_runtime_value(&function_name)
                                .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                        );
                    }
                }
            } else {
                self.emit_numeric_expression(&fallback_value)?;
            }
            self.push_global_set(target_binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(target_binding.present_index);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_scoped_property_read(
        &mut self,
        scope_object: &Expression,
        name: &str,
    ) -> DirectResult<()> {
        let property = Expression::String(name.to_string());
        if let Some(function_binding) =
            self.resolve_member_function_binding(scope_object, &property)
        {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.module.user_function_map.get(&function_name) {
                        self.push_i32_const(user_function_runtime_value(user_function));
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(_) => {
                    self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                }
            }
            return Ok(());
        }
        if let Some(getter_binding) = self.resolve_member_getter_binding(scope_object, &property) {
            match getter_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) =
                        self.module.user_function_map.get(&function_name).cloned()
                    {
                        self.emit_user_function_call_with_function_this_binding(
                            &user_function,
                            &[],
                            scope_object,
                            None,
                        )?;
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    let callee = Expression::Identifier(function_name);
                    if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
            }
            return Ok(());
        }
        if let Some(binding) =
            self.resolve_runtime_object_property_shadow_binding(scope_object, &property)
        {
            let fallback_value = self
                .resolve_object_binding_from_expression(scope_object)
                .and_then(|object_binding| {
                    object_binding_lookup_value(&object_binding, &property).cloned()
                });
            self.push_global_get(binding.present_index);
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.push_global_get(binding.value_index);
            self.instructions.push(0x05);
            if let Some(fallback_value) = fallback_value {
                self.emit_numeric_expression(&fallback_value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(scope_object) {
            if let Some(value) = object_binding_lookup_value(&object_binding, &property) {
                self.emit_numeric_expression(value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(());
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_object_spread_copy_data_properties_effects(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        if !inline_summary_side_effect_free_expression(expression) {
            return Ok(());
        }
        let Some(object_binding) = self.resolve_object_binding_from_expression(expression) else {
            return Ok(());
        };

        for property_name in ordered_object_property_names(&object_binding) {
            if object_binding
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == &property_name)
            {
                continue;
            }
            self.emit_member_read_without_prelude(expression, &Expression::String(property_name))?;
            self.instructions.push(0x1a);
        }
        for (property, _) in &object_binding.symbol_properties {
            self.emit_member_read_without_prelude(expression, property)?;
            self.instructions.push(0x1a);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_length(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<u32> {
        if !matches!(property, Expression::String(property_name) if property_name == "length") {
            return None;
        }
        match self.resolve_function_binding_from_expression(object)? {
            LocalFunctionBinding::User(function_name) => self
                .module
                .user_function_map
                .get(&function_name)
                .map(|user_function| user_function.length),
            LocalFunctionBinding::Builtin(function_name) => builtin_function_length(&function_name),
        }
    }

    pub(in crate::backend::direct_wasm) fn runtime_user_function_property_value(
        &self,
        user_function: &UserFunction,
        property_name: &str,
    ) -> Option<Expression> {
        let property = Expression::String(property_name.to_string());
        if let Some(object_binding) = self.module.global_object_bindings.get(&user_function.name)
            && let Some(value) = object_binding_lookup_value(object_binding, &property)
        {
            match value {
                Expression::Identifier(name)
                    if property_name == "name" && name == &user_function.name => {}
                Expression::String(_) | Expression::Number(_) | Expression::Identifier(_) => {
                    return Some(value.clone());
                }
                _ => return None,
            }
        }
        match property_name {
            "name" => self
                .resolve_registered_function_declaration(&user_function.name)
                .map(|function| {
                    Expression::String(function_display_name(function).unwrap_or_default())
                }),
            "length" => Some(Expression::Number(user_function.length as f64)),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_user_function_property_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };

        let mut candidates = Vec::new();
        for user_function in &self.module.user_functions {
            let Some(value) =
                self.runtime_user_function_property_value(user_function, property_name)
            else {
                continue;
            };
            candidates.push((user_function_runtime_value(user_function), value));
        }
        if candidates.is_empty() {
            return Ok(false);
        }

        let object_local = self.allocate_temp_local();
        let result_local = self.allocate_temp_local();
        let matched_local = self.allocate_temp_local();
        self.emit_numeric_expression(object)?;
        self.push_local_set(object_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_local_set(result_local);
        self.push_i32_const(0);
        self.push_local_set(matched_local);

        for (runtime_value, value) in candidates {
            self.push_local_get(object_local);
            self.push_i32_const(runtime_value);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_numeric_expression(&value)?;
            self.push_local_set(result_local);
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(matched_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(result_local);
        self.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_constructed_object_constructor_binding(
        &self,
        object: &Expression,
    ) -> Option<LocalFunctionBinding> {
        if let Some(binding) = self
            .resolve_member_function_binding(object, &Expression::String("constructor".to_string()))
        {
            return Some(binding);
        }
        let materialized_object = self.materialize_static_expression(object);
        match &materialized_object {
            Expression::New { callee, .. } => self.resolve_function_binding_from_expression(callee),
            _ if !static_expression_matches(&materialized_object, object) => {
                self.resolve_constructed_object_constructor_binding(&materialized_object)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_name_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<String> {
        if !matches!(property, Expression::String(property_name) if property_name == "name") {
            return None;
        }
        match self.resolve_function_binding_from_expression(object)? {
            LocalFunctionBinding::User(function_name) => self
                .resolve_registered_function_declaration(&function_name)
                .map(|function| function_display_name(function).unwrap_or_default()),
            LocalFunctionBinding::Builtin(function_name) => {
                Some(builtin_function_display_name(&function_name).to_string())
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_typed_array_builtin_bytes_per_element(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<u32> {
        if !matches!(property, Expression::String(property_name) if property_name == "BYTES_PER_ELEMENT")
        {
            return None;
        }
        let Expression::Identifier(name) = self.materialize_static_expression(object) else {
            return None;
        };
        typed_array_builtin_bytes_per_element(&name)
    }
}
