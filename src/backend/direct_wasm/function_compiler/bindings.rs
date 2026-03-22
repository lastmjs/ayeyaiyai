use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn with_suspended_with_scopes<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let previous_with_scopes = std::mem::take(&mut self.with_scopes);
        let result = f(self);
        self.with_scopes = previous_with_scopes;
        result
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_function_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key.and_then(|key| {
            self.member_function_bindings
                .get(&key)
                .cloned()
                .or_else(|| {
                    self.module
                        .global_member_function_bindings
                        .get(&key)
                        .cloned()
                })
        });
        if resolved.is_some() {
            return resolved;
        }

        let materialized_property = self.materialize_static_expression(property);
        match object {
            Expression::Identifier(name) => {
                if let Some(index) = argument_index_from_expression(&materialized_property) {
                    if let Some(binding) = self
                        .tracked_array_function_values
                        .get(name)
                        .and_then(|bindings| bindings.get(&index))
                        .map(|value| value.binding.clone())
                    {
                        return Some(binding);
                    }
                    if let Some(value) = self
                        .local_array_bindings
                        .get(name)
                        .or_else(|| self.module.global_array_bindings.get(name))
                        .and_then(|array_binding| array_binding.values.get(index as usize))
                        .cloned()
                        .flatten()
                    {
                        return self.resolve_function_binding_from_expression(&value);
                    }
                }
                self.local_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &materialized_property)
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
                    .or_else(|| {
                        self.resolve_object_binding_from_expression(object)
                            .and_then(|object_binding| {
                                object_binding_lookup_value(&object_binding, &materialized_property)
                                    .cloned()
                            })
                            .and_then(|value| self.resolve_function_binding_from_expression(&value))
                    })
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                self.local_prototype_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_prototype_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &materialized_property)
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                self.local_prototype_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_prototype_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &materialized_property)
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
            }
            _ => self
                .resolve_object_binding_from_expression(object)
                .and_then(|object_binding| {
                    object_binding_lookup_value(&object_binding, &materialized_property).cloned()
                })
                .and_then(|value| self.resolve_function_binding_from_expression(&value)),
        };
        if resolved.is_some() {
            return resolved;
        }

        let materialized_object = self.materialize_static_expression(object);
        if !static_expression_matches(&materialized_object, object)
            || !static_expression_matches(&materialized_property, property)
        {
            return self
                .resolve_member_function_binding(&materialized_object, &materialized_property);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_getter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key.and_then(|key| {
            self.member_getter_bindings
                .get(&key)
                .cloned()
                .or_else(|| self.module.global_member_getter_bindings.get(&key).cloned())
        });
        if resolved.is_some() {
            return resolved;
        }

        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self.materialize_static_expression(property);
        if !static_expression_matches(&materialized_object, object)
            || !static_expression_matches(&materialized_property, property)
        {
            return self
                .resolve_member_getter_binding(&materialized_object, &materialized_property);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_setter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key.and_then(|key| {
            self.member_setter_bindings
                .get(&key)
                .cloned()
                .or_else(|| self.module.global_member_setter_bindings.get(&key).cloned())
        });
        if resolved.is_some() {
            return resolved;
        }

        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self.materialize_static_expression(property);
        if !static_expression_matches(&materialized_object, object)
            || !static_expression_matches(&materialized_property, property)
        {
            return self
                .resolve_member_setter_binding(&materialized_object, &materialized_property);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_proxy_has_binding_from_handler(
        &self,
        handler: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let property = Expression::String("has".to_string());
        match handler {
            Expression::Identifier(name) => {
                let key = MemberFunctionBindingKey {
                    target: MemberFunctionBindingTarget::Identifier(name.clone()),
                    property: MemberFunctionBindingProperty::String("has".to_string()),
                };
                self.member_function_bindings
                    .get(&key)
                    .cloned()
                    .or_else(|| {
                        self.module
                            .global_member_function_bindings
                            .get(&key)
                            .cloned()
                    })
                    .or_else(|| {
                        self.resolve_object_binding_from_expression(handler)
                            .and_then(|object_binding| {
                                object_binding_lookup_value(&object_binding, &property).and_then(
                                    |value| self.resolve_function_binding_from_expression(value),
                                )
                            })
                    })
            }
            Expression::Object(entries) => entries.iter().find_map(|entry| {
                let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                    return None;
                };
                let key = self
                    .resolve_property_key_expression(key)
                    .unwrap_or_else(|| self.materialize_static_expression(key));
                if !matches!(key, Expression::String(ref name) if name == "has") {
                    return None;
                }
                self.resolve_function_binding_from_expression(value)
            }),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_proxy_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ProxyValueBinding> {
        match expression {
            Expression::Identifier(name) => self
                .local_proxy_bindings
                .get(name)
                .cloned()
                .or_else(|| self.module.global_proxy_bindings.get(name).cloned()),
            Expression::New { callee, arguments } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Proxy" && self.is_unshadowed_builtin_identifier(name)) =>
            {
                let [
                    CallArgument::Expression(target),
                    CallArgument::Expression(handler),
                    ..,
                ] = arguments.as_slice()
                else {
                    return None;
                };
                Some(ProxyValueBinding {
                    target: self.materialize_static_expression(target),
                    has_binding: self.resolve_proxy_has_binding_from_handler(handler),
                })
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_proxy_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(proxy_binding) = self.resolve_proxy_binding_from_expression(value) else {
            self.local_proxy_bindings.remove(name);
            if self.binding_name_is_global(name) {
                self.module.global_proxy_bindings.remove(name);
            }
            return;
        };
        self.local_proxy_bindings
            .insert(name.to_string(), proxy_binding.clone());
        if self.binding_name_is_global(name) {
            self.module
                .global_proxy_bindings
                .insert(name.to_string(), proxy_binding);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

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

        let source_name = scoped_binding_source_name(name)?;
        bindings.get(source_name).cloned()
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

        self.sync_user_function_capture_static_metadata(name, &hidden_name);
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

            let active_name = active_scoped_lexical_bindings
                .get(name)
                .and_then(|bindings| bindings.last())
                .cloned()?;
            locals
                .get(&active_name)
                .copied()
                .map(|local_index| (active_name, local_index))
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
            let callee = match getter_binding {
                LocalFunctionBinding::User(function_name)
                | LocalFunctionBinding::Builtin(function_name) => {
                    Expression::Identifier(function_name)
                }
            };
            if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
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
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(object)?
        else {
            return None;
        };
        self.module
            .user_function_map
            .get(&function_name)
            .map(|user_function| user_function.length)
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
            return match value {
                Expression::String(_) | Expression::Number(_) | Expression::Identifier(_) => {
                    Some(value.clone())
                }
                _ => None,
            };
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

    pub(in crate::backend::direct_wasm) fn emit_member_read_without_prelude(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<()> {
        if matches!(property, Expression::String(property_name) if property_name == "global")
            && matches!(
                object,
                Expression::Call { callee, arguments }
                    if arguments.is_empty()
                        && matches!(
                            callee.as_ref(),
                            Expression::Member { object, property }
                                if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                    && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                        )
            )
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        if let Some(text) = self.resolve_static_string_value(&Expression::Member {
            object: Box::new(self.materialize_static_expression(object)),
            property: Box::new(self.materialize_static_expression(property)),
        }) {
            self.emit_static_string_literal(&text)?;
            return Ok(());
        }
        if matches!(object, Expression::Identifier(name) if name == "Number" && self.is_unshadowed_builtin_identifier(name))
            && matches!(property, Expression::String(property_name) if property_name == "NaN")
        {
            self.push_i32_const(JS_NAN_TAG);
            return Ok(());
        }
        if let Some(step_binding) = self.resolve_iterator_step_binding_from_expression(object) {
            if let Expression::String(property_name) = property {
                match property_name.as_str() {
                    "done" => {
                        match step_binding {
                            IteratorStepBinding::Runtime { done_local, .. } => {
                                self.push_local_get(done_local);
                            }
                        }
                        return Ok(());
                    }
                    "value" => {
                        match step_binding {
                            IteratorStepBinding::Runtime { value_local, .. } => {
                                self.push_local_get(value_local);
                            }
                        }
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        if let Expression::Identifier(name) = object {
            if self.local_typed_array_view_bindings.contains_key(name) {
                if matches!(property, Expression::String(text) if text == "length") {
                    if let Some(length_local) = self.runtime_array_length_locals.get(name).copied()
                    {
                        self.push_local_get(length_local);
                    } else {
                        self.push_i32_const(0);
                    }
                    return Ok(());
                }
                if let Some(index) = argument_index_from_expression(property) {
                    if let Some(oob_local) = self.runtime_typed_array_oob_locals.get(name).copied()
                    {
                        self.push_local_get(oob_local);
                        self.instructions.push(0x04);
                        self.instructions.push(I32_TYPE);
                        self.push_control_frame();
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        self.instructions.push(0x05);
                        if !self.emit_runtime_array_slot_read(name, index)? {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                        }
                        self.instructions.push(0x0b);
                        self.pop_control_frame();
                    } else if !self.emit_runtime_array_slot_read(name, index)? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                    return Ok(());
                }
            }
        }
        if let Some(bytes_per_element) =
            self.resolve_typed_array_builtin_bytes_per_element(object, property)
        {
            self.push_i32_const(bytes_per_element as i32);
            return Ok(());
        }
        if let Some(function_name) = self.resolve_function_name_value(object, property) {
            self.emit_static_string_literal(&function_name)?;
            return Ok(());
        }
        if let Some(function_length) = self.resolve_user_function_length(object, property) {
            self.push_i32_const(function_length as i32);
            return Ok(());
        }
        if let Some(function_binding) = self.resolve_member_function_binding(object, property) {
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
        if let Some(function_binding) = self.resolve_member_getter_binding(object, property) {
            let callee = match function_binding {
                LocalFunctionBinding::User(function_name)
                | LocalFunctionBinding::Builtin(function_name) => {
                    Expression::Identifier(function_name)
                }
            };
            if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(());
        }
        if matches!(property, Expression::String(property_name) if property_name == "caller") {
            if let Some(strict) = self.resolve_arguments_callee_strictness(object) {
                if strict {
                    return self.emit_error_throw();
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(());
            }
        }
        if self.is_restricted_arrow_function_property(object, property) {
            self.emit_numeric_expression(object)?;
            self.instructions.push(0x1a);
            return self.emit_named_error_throw("TypeError");
        }
        if let Expression::Identifier(name) = object {
            if let Some(descriptor) = self.local_descriptor_bindings.get(name) {
                if let Expression::String(property_name) = property {
                    match property_name.as_str() {
                        "value" => {
                            if let Some(value) = descriptor.value.clone() {
                                self.emit_numeric_expression(&value)?;
                            } else {
                                self.push_i32_const(JS_UNDEFINED_TAG);
                            }
                            return Ok(());
                        }
                        "configurable" => {
                            self.push_i32_const(if descriptor.configurable { 1 } else { 0 });
                            return Ok(());
                        }
                        "enumerable" => {
                            self.push_i32_const(if descriptor.enumerable { 1 } else { 0 });
                            return Ok(());
                        }
                        "writable" => {
                            if let Some(writable) = descriptor.writable {
                                self.push_i32_const(if writable { 1 } else { 0 });
                            } else {
                                self.push_i32_const(JS_UNDEFINED_TAG);
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            if let Some(index) = argument_index_from_expression(property) {
                if self.emit_runtime_array_slot_read(name, index)? {
                    return Ok(());
                }
            }
        }
        if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                if let Some(length_local) = self.runtime_array_length_local_for_expression(object) {
                    self.push_local_get(length_local);
                } else {
                    self.push_i32_const(array_binding.values.len() as i32);
                }
                return Ok(());
            }
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(Some(value)) = array_binding.values.get(index as usize) {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
            if let Some(value) =
                self.resolve_object_binding_property_value(&object_binding, property)
            {
                self.emit_numeric_expression(&value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(());
        }
        if let Expression::String(text) = object {
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(character) = text.chars().nth(index as usize) {
                    self.emit_numeric_expression(&Expression::String(character.to_string()))?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
            if matches!(property, Expression::String(name) if name == "length") {
                self.push_i32_const(text.chars().count() as i32);
                return Ok(());
            }
        }
        if let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                if !arguments_binding.length_present {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                } else {
                    self.emit_numeric_expression(&arguments_binding.length_value)?;
                }
                return Ok(());
            }
            if matches!(property, Expression::String(property_name) if property_name == "callee") {
                if arguments_binding.strict {
                    return self.emit_error_throw();
                }
                if !arguments_binding.callee_present {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                } else if let Some(value) = arguments_binding.callee_value.as_ref() {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(value) = arguments_binding.values.get(index as usize) {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
            return self.emit_dynamic_arguments_binding_property_read(&arguments_binding, property);
        }
        if self.is_direct_arguments_object(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                return self.emit_direct_arguments_length();
            }
            if matches!(property, Expression::String(text) if text == "callee") {
                return self.emit_direct_arguments_callee();
            }
            if let Some(index) = argument_index_from_expression(property) {
                return self.emit_arguments_slot_read(index);
            }
            return self.emit_dynamic_direct_arguments_property_read(property);
        }
        if let Some(returned_value) =
            self.resolve_returned_member_value_from_expression(object, property)
        {
            self.emit_numeric_expression(&returned_value)?;
            return Ok(());
        }
        if self.emit_runtime_user_function_property_read(object, property)? {
            return Ok(());
        }
        if matches!(property, Expression::String(text) if text == "constructor") {
            self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
            return Ok(());
        }
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_scoped_property_store_from_local(
        &mut self,
        scope_object: &Expression,
        name: &str,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<()> {
        let property = Expression::String(name.to_string());
        if let Some(setter_binding) = self.resolve_member_setter_binding(scope_object, &property) {
            let inline_value = self.materialize_static_expression(value_expression);
            let callee = match setter_binding {
                LocalFunctionBinding::User(function_name)
                | LocalFunctionBinding::Builtin(function_name) => {
                    Expression::Identifier(function_name)
                }
            };
            if self.emit_arguments_slot_accessor_call(
                &callee,
                &[value_local],
                1,
                Some(std::slice::from_ref(&inline_value)),
            )? {
                self.instructions.push(0x1a);
            }
            self.push_local_get(value_local);
            return Ok(());
        }

        let materialized_value = self.materialize_static_expression(value_expression);
        if let Expression::Identifier(scope_name) = scope_object {
            if let Some(object_binding) = self.local_object_bindings.get_mut(scope_name) {
                object_binding_set_property(
                    object_binding,
                    property.clone(),
                    materialized_value.clone(),
                );
                self.push_local_get(value_local);
                return Ok(());
            }
            if let Some(object_binding) = self.module.global_object_bindings.get_mut(scope_name) {
                object_binding_set_property(object_binding, property, materialized_value);
                self.push_local_get(value_local);
                return Ok(());
            }
        }

        self.push_local_get(value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_scoped_property_update(
        &mut self,
        scope_object: &Expression,
        name: &str,
        op: UpdateOp,
        prefix: bool,
    ) -> DirectResult<()> {
        let opcode = match op {
            UpdateOp::Increment => 0x6a,
            UpdateOp::Decrement => 0x6b,
        };
        let property = Expression::String(name.to_string());
        let member_expression = Expression::Member {
            object: Box::new(scope_object.clone()),
            property: Box::new(property.clone()),
        };
        let previous_kind = self
            .infer_value_kind(&member_expression)
            .unwrap_or(StaticValueKind::Unknown);
        let current_value = self
            .resolve_object_binding_from_expression(scope_object)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &property).cloned()
            })
            .unwrap_or(Expression::Undefined);
        let increment = match op {
            UpdateOp::Increment => 1.0,
            UpdateOp::Decrement => -1.0,
        };

        match previous_kind {
            StaticValueKind::Undefined
            | StaticValueKind::String
            | StaticValueKind::Object
            | StaticValueKind::Function
            | StaticValueKind::Symbol
            | StaticValueKind::BigInt => {
                let nan_local = self.allocate_temp_local();
                self.push_i32_const(JS_NAN_TAG);
                self.push_local_set(nan_local);
                self.emit_scoped_property_store_from_local(
                    scope_object,
                    name,
                    nan_local,
                    &Expression::Number(f64::NAN),
                )?;
                self.instructions.push(0x1a);
                self.push_local_get(nan_local);
                return Ok(());
            }
            StaticValueKind::Null => {
                let previous_local = self.allocate_temp_local();
                let next_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(previous_local);
                self.push_i32_const(increment as i32);
                self.push_local_set(next_local);
                self.emit_scoped_property_store_from_local(
                    scope_object,
                    name,
                    next_local,
                    &Expression::Number(increment),
                )?;
                self.instructions.push(0x1a);
                if prefix {
                    self.push_local_get(next_local);
                } else {
                    self.push_local_get(previous_local);
                }
                return Ok(());
            }
            _ => {}
        }

        let previous_local = self.allocate_temp_local();
        let next_local = self.allocate_temp_local();
        self.emit_scoped_property_read(scope_object, name)?;
        self.push_local_set(previous_local);
        self.push_local_get(previous_local);
        self.push_i32_const(1);
        self.instructions.push(opcode);
        self.push_local_set(next_local);
        let next_expression = match previous_kind {
            StaticValueKind::Bool => {
                let previous = match self.materialize_static_expression(&current_value) {
                    Expression::Bool(value) => {
                        if value {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    _ => 0.0,
                };
                Expression::Number(previous + increment)
            }
            _ => self
                .resolve_static_number_value(&current_value)
                .map(|value| Expression::Number(value + increment))
                .unwrap_or(Expression::Number(f64::NAN)),
        };
        self.emit_scoped_property_store_from_local(
            scope_object,
            name,
            next_local,
            &next_expression,
        )?;
        self.instructions.push(0x1a);
        if prefix {
            self.push_local_get(next_local);
        } else {
            self.push_local_get(previous_local);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_home_object_name_for_function(
        &self,
        function_name: &str,
    ) -> Option<String> {
        if let Some(home_object_name) = self
            .module
            .user_function_map
            .get(function_name)?
            .home_object_binding
            .as_ref()
        {
            return Some(home_object_name.clone());
        }
        self.module.global_value_bindings.iter().find_map(|(name, value)| {
            let Expression::Object(entries) = value else {
                return None;
            };
            entries.iter().find_map(|entry| {
                let candidate = match entry {
                    crate::ir::hir::ObjectEntry::Data { value, .. } => value,
                    crate::ir::hir::ObjectEntry::Getter { getter, .. } => getter,
                    crate::ir::hir::ObjectEntry::Setter { setter, .. } => setter,
                    crate::ir::hir::ObjectEntry::Spread(_) => return None,
                };
                matches!(candidate, Expression::Identifier(candidate_name) if candidate_name == function_name)
                    .then_some(name.clone())
            })
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_base_expression_with_context(
        &self,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let function_name = current_function_name?;
        let home_object_name = self.resolve_home_object_name_for_function(function_name)?;
        self.module
            .global_object_prototype_bindings
            .get(&home_object_name)
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_runtime_prototype_binding_with_context(
        &self,
        current_function_name: Option<&str>,
    ) -> Option<(String, GlobalObjectRuntimePrototypeBinding)> {
        let function_name = current_function_name?;
        let home_object_name = self.resolve_home_object_name_for_function(function_name)?;
        let binding = self
            .module
            .global_runtime_prototype_bindings
            .get(&home_object_name)?
            .clone();
        Some((home_object_name, binding))
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_super_property_value_from_base(
        &mut self,
        base: Option<&Expression>,
        property: &Expression,
    ) -> DirectResult<()> {
        let Some(base) = base else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };
        if let Some(function_binding) = self.resolve_member_function_binding(base, property) {
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
        if let Some(function_binding) = self.resolve_member_getter_binding(base, property) {
            let callee = match function_binding {
                LocalFunctionBinding::User(function_name)
                | LocalFunctionBinding::Builtin(function_name) => {
                    Expression::Identifier(function_name)
                }
            };
            if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(());
        }
        let materialized_property = self.materialize_static_expression(property);
        if let Some(object_binding) = self.resolve_object_binding_from_expression(base)
            && let Some(value) =
                object_binding_lookup_value(&object_binding, &materialized_property).cloned()
        {
            self.emit_numeric_expression(&value)?;
            return Ok(());
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_super_member_read_via_runtime_prototype_binding(
        &mut self,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Some((_, binding)) = self.resolve_super_runtime_prototype_binding_with_context(
            self.current_user_function_name.as_deref(),
        ) else {
            return Ok(false);
        };
        let Some(global_index) = binding.global_index else {
            return Ok(false);
        };
        let resolved_property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        if !matches!(
            resolved_property,
            Expression::String(_) | Expression::Identifier(_) | Expression::Member { .. }
        ) {
            return Ok(false);
        }

        let state_local = self.allocate_temp_local();
        self.push_global_get(global_index);
        self.push_local_set(state_local);

        let mut open_frames = 0;
        for (variant_index, prototype) in binding.variants.iter().enumerate() {
            self.push_local_get(state_local);
            self.push_i32_const(variant_index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_runtime_super_property_value_from_base(
                prototype.as_ref(),
                &resolved_property,
            )?;
            self.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_function_binding(
        &self,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_super_function_binding_with_context(
            property,
            self.current_user_function_name.as_deref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_function_binding_with_context(
        &self,
        property: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let base = self.resolve_super_base_expression_with_context(current_function_name)?;
        let materialized_property = self.materialize_static_expression(property);
        self.resolve_member_function_binding(&base, property)
            .or_else(|| {
                self.resolve_object_binding_from_expression(&base)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &materialized_property)
                            .cloned()
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(&value))
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_getter_binding(
        &self,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let base = self.resolve_super_base_expression_with_context(
            self.current_user_function_name.as_deref(),
        )?;
        self.resolve_member_getter_binding(&base, property)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_value_expression(
        &self,
        property: &Expression,
    ) -> Option<Expression> {
        let base = self.resolve_super_base_expression_with_context(
            self.current_user_function_name.as_deref(),
        )?;
        let materialized_property = self.materialize_static_expression(property);
        self.resolve_object_binding_from_expression(&base)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &materialized_property).cloned()
            })
    }

    pub(in crate::backend::direct_wasm) fn binding_name_is_global(&self, name: &str) -> bool {
        self.top_level_function
            && self.module.global_bindings.contains_key(name)
            && !self.locals.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn binding_key_is_global(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> bool {
        match &key.target {
            MemberFunctionBindingTarget::Identifier(name)
            | MemberFunctionBindingTarget::Prototype(name) => self.binding_name_is_global(name),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_named_function_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
        descriptor_name: &str,
    ) -> Option<LocalFunctionBinding> {
        let Expression::Object(entries) = descriptor else {
            return None;
        };
        for entry in entries {
            let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                continue;
            };
            if matches!(key, Expression::String(name) if name == descriptor_name) {
                return self.resolve_function_binding_from_expression(value);
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_named_function_binding_from_descriptor_expression(descriptor, "value")
    }

    pub(in crate::backend::direct_wasm) fn resolve_getter_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_named_function_binding_from_descriptor_expression(descriptor, "get")
    }

    pub(in crate::backend::direct_wasm) fn resolve_setter_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_named_function_binding_from_descriptor_expression(descriptor, "set")
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_binding_from_expression(
        &mut self,
        expression: &Expression,
    ) {
        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
            return;
        }

        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };

        let Some(key) = self.member_function_binding_key(target, property) else {
            return;
        };
        let value_binding = self.resolve_function_binding_from_descriptor_expression(descriptor);
        let getter_binding = self.resolve_getter_binding_from_descriptor_expression(descriptor);
        let setter_binding = self.resolve_setter_binding_from_descriptor_expression(descriptor);

        if let Some(binding) = value_binding {
            self.member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_function_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_function_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_function_bindings.remove(&key);
            }
        }

        if let Some(binding) = getter_binding {
            self.member_getter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_getter_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_getter_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_getter_bindings.remove(&key);
            }
        }

        if let Some(binding) = setter_binding {
            self.member_setter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_setter_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_setter_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_setter_bindings.remove(&key);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_assignment_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let Some(key) = self.member_function_binding_key(object, property) else {
            return;
        };
        let value_binding = self.resolve_function_binding_from_expression(value);

        if let Some(binding) = value_binding {
            self.member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_function_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_function_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_function_bindings.remove(&key);
            }
        }

        self.member_getter_bindings.remove(&key);
        self.member_setter_bindings.remove(&key);
        if self.binding_key_is_global(&key) {
            self.module.global_member_getter_bindings.remove(&key);
            self.module.global_member_setter_bindings.remove(&key);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_function_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(function_name) = self.resolve_function_binding_from_expression(value) else {
            self.local_function_bindings.remove(name);
            return;
        };
        self.local_function_bindings
            .insert(name.to_string(), function_name);
    }

    pub(in crate::backend::direct_wasm) fn clear_member_function_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        self.member_function_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        self.member_getter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        self.member_setter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        if self.binding_name_is_global(name) {
            self.module.global_member_function_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
            self.module.global_member_getter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
            self.module.global_member_setter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_object_literal_member_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        self.member_function_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.member_getter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.member_setter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        if self.binding_name_is_global(name) {
            self.module.global_member_function_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
            self.module.global_member_getter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
            self.module.global_member_setter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
        }
    }

    pub(in crate::backend::direct_wasm) fn object_literal_member_function_bindings(
        &self,
        entries: &[crate::ir::hir::ObjectEntry],
    ) -> Vec<ReturnedMemberFunctionBinding> {
        entries
            .iter()
            .filter_map(|entry| {
                let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                    return None;
                };
                let Expression::String(property) = key else {
                    return None;
                };
                let binding = self.resolve_function_binding_from_expression(value)?;
                Some(ReturnedMemberFunctionBinding {
                    target: ReturnedMemberFunctionBindingTarget::Value,
                    property: property.clone(),
                    binding,
                })
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn inherited_member_function_bindings(
        &self,
        value: &Expression,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        match value {
            Expression::Identifier(source_name) => self
                .member_function_bindings
                .iter()
                .chain(self.module.global_member_function_bindings.iter())
                .filter_map(|(key, binding)| match &key.target {
                    MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Value,
                            property: property.clone(),
                            binding: binding.clone(),
                        })
                    }
                    MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Prototype,
                            property: property.clone(),
                            binding: binding.clone(),
                        })
                    }
                    _ => None,
                })
                .collect(),
            Expression::Call { callee, .. } | Expression::New { callee, .. } => {
                let Expression::Identifier(callee_name) = callee.as_ref() else {
                    return Vec::new();
                };
                let Some(user_function) = self.resolve_user_function_from_callee_name(callee_name)
                else {
                    return Vec::new();
                };
                user_function.returned_member_function_bindings.clone()
            }
            Expression::Object(entries) => self.object_literal_member_function_bindings(entries),
            _ => Vec::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn copy_member_bindings_for_alias(
        &mut self,
        name: &str,
        source_name: &str,
    ) {
        let mut function_bindings = Vec::new();
        let mut getter_bindings = Vec::new();
        let mut setter_bindings = Vec::new();

        for (key, binding) in self
            .member_function_bindings
            .iter()
            .chain(self.module.global_member_function_bindings.iter())
        {
            let target = match &key.target {
                MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
                }
                MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                function_bindings.push((
                    MemberFunctionBindingKey {
                        target,
                        property: key.property.clone(),
                    },
                    binding.clone(),
                ));
            }
        }

        for (key, binding) in self
            .member_getter_bindings
            .iter()
            .chain(self.module.global_member_getter_bindings.iter())
        {
            let target = match &key.target {
                MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
                }
                MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                getter_bindings.push((
                    MemberFunctionBindingKey {
                        target,
                        property: key.property.clone(),
                    },
                    binding.clone(),
                ));
            }
        }

        for (key, binding) in self
            .member_setter_bindings
            .iter()
            .chain(self.module.global_member_setter_bindings.iter())
        {
            let target = match &key.target {
                MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
                }
                MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                setter_bindings.push((
                    MemberFunctionBindingKey {
                        target,
                        property: key.property.clone(),
                    },
                    binding.clone(),
                ));
            }
        }

        for (key, binding) in function_bindings {
            self.member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_function_bindings
                    .insert(key, binding);
            }
        }
        for (key, binding) in getter_bindings {
            self.member_getter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_getter_bindings
                    .insert(key, binding);
            }
        }
        for (key, binding) in setter_bindings {
            self.member_setter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_setter_bindings
                    .insert(key, binding);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        self.clear_member_function_bindings_for_name(name);
        if let Expression::Identifier(source_name) = value {
            self.copy_member_bindings_for_alias(name, source_name);
        }
        for binding in self.inherited_member_function_bindings(value) {
            let target = match binding.target {
                ReturnedMemberFunctionBindingTarget::Value => {
                    MemberFunctionBindingTarget::Identifier(name.to_string())
                }
                ReturnedMemberFunctionBindingTarget::Prototype => {
                    MemberFunctionBindingTarget::Prototype(name.to_string())
                }
            };
            let key = MemberFunctionBindingKey {
                target,
                property: MemberFunctionBindingProperty::String(binding.property),
            };
            self.member_function_bindings
                .insert(key.clone(), binding.binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_function_bindings
                    .insert(key, binding.binding);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn update_object_literal_member_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Object(entries) = value else {
            return;
        };

        self.clear_object_literal_member_bindings_for_name(name);

        let mut states: HashMap<
            MemberFunctionBindingProperty,
            (
                Option<LocalFunctionBinding>,
                Option<LocalFunctionBinding>,
                Option<LocalFunctionBinding>,
            ),
        > = HashMap::new();

        for entry in entries {
            let (key, binding, slot) = match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    (key, self.resolve_function_binding_from_expression(value), 0)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => (
                    key,
                    self.resolve_function_binding_from_expression(getter),
                    1,
                ),
                crate::ir::hir::ObjectEntry::Setter { key, setter } => (
                    key,
                    self.resolve_function_binding_from_expression(setter),
                    2,
                ),
                crate::ir::hir::ObjectEntry::Spread(_) => return,
            };

            let materialized_key = self
                .resolve_property_key_expression(key)
                .unwrap_or_else(|| self.materialize_static_expression(key));
            let Some(property_name) = self.member_function_binding_property(&materialized_key)
            else {
                continue;
            };
            let state = states.entry(property_name).or_insert((None, None, None));
            match slot {
                0 => {
                    state.0 = binding;
                    state.1 = None;
                    state.2 = None;
                }
                1 => {
                    state.0 = None;
                    state.1 = binding;
                }
                2 => {
                    state.0 = None;
                    state.2 = binding;
                }
                _ => {}
            }
        }

        for (property, (value_binding, getter_binding, setter_binding)) in states {
            let key = MemberFunctionBindingKey {
                target: MemberFunctionBindingTarget::Identifier(name.to_string()),
                property,
            };
            if let Some(binding) = value_binding {
                self.member_function_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.module
                        .global_member_function_bindings
                        .insert(key.clone(), binding);
                }
            }
            if let Some(binding) = getter_binding {
                self.member_getter_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.module
                        .global_member_getter_bindings
                        .insert(key.clone(), binding);
                }
            }
            if let Some(binding) = setter_binding {
                self.member_setter_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.module
                        .global_member_setter_bindings
                        .insert(key, binding);
                }
            }
        }
    }

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
        } else if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
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
        let canonical_value_expression = self
            .prepare_special_assignment_expression(value_expression)
            .unwrap_or_else(|| value_expression.clone());
        let function_binding =
            self.resolve_function_binding_from_expression(&canonical_value_expression);
        let kind = self.infer_value_kind(&canonical_value_expression);
        let static_string_value = self.resolve_static_string_value(&canonical_value_expression);
        let resolved_local_binding = self.resolve_current_local_binding(name);
        if self.isolated_indirect_eval
            && resolved_local_binding.is_none()
            && self.parameter_scope_arguments_local_for(name).is_none()
        {
            if let Some(global_index) = self.module.global_bindings.get(name).copied() {
                self.module
                    .update_static_global_assignment_metadata(name, &canonical_value_expression);
                if let Some(text) = static_string_value.clone() {
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
                self.update_global_specialized_function_value(name, &canonical_value_expression)?;
                self.update_global_property_descriptor_value(name, &canonical_value_expression);
                self.push_local_get(value_local);
                self.push_global_set(global_index);
                return Ok(());
            }
            if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
                return Ok(());
            }
            if let Some(binding) = self.module.implicit_global_bindings.get(name).copied() {
                self.module
                    .update_static_global_assignment_metadata(name, &canonical_value_expression);
                if let Some(text) = static_string_value.clone() {
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
                self.update_global_specialized_function_value(name, &canonical_value_expression)?;
                self.ensure_global_property_descriptor_value(
                    name,
                    &canonical_value_expression,
                    true,
                );
                self.emit_store_implicit_global_from_local(binding, value_local)?;
                return Ok(());
            }
            let binding = self.module.ensure_implicit_global_binding(name);
            self.module
                .update_static_global_assignment_metadata(name, &canonical_value_expression);
            if let Some(text) = static_string_value.clone() {
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
            self.update_global_specialized_function_value(name, &canonical_value_expression)?;
            self.ensure_global_property_descriptor_value(name, &canonical_value_expression, true);
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
        if !is_internal_iterator_temp {
            self.update_local_function_binding(resolved_name, &canonical_value_expression);
            self.update_local_specialized_function_value(
                resolved_name,
                &canonical_value_expression,
            )?;
            self.update_local_proxy_binding(resolved_name, &canonical_value_expression);
            self.update_member_function_bindings_for_value(
                resolved_name,
                &canonical_value_expression,
            );
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
        if !is_internal_iterator_temp {
            self.update_local_object_binding(resolved_name, &canonical_value_expression);
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
            self.emit_store_user_function_capture_binding_from_local(name, value_local)?;
        } else if let Some(global_index) = self.module.global_bindings.get(name).copied() {
            if !self.isolated_indirect_eval {
                self.module
                    .update_static_global_assignment_metadata(name, &canonical_value_expression);
                if let Some(text) = static_string_value.clone() {
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
                self.update_global_specialized_function_value(name, &canonical_value_expression)?;
                self.update_global_property_descriptor_value(name, &canonical_value_expression);
            }
            self.push_local_get(value_local);
            self.push_global_set(global_index);
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
                    .update_static_global_assignment_metadata(name, &canonical_value_expression);
                if let Some(text) = static_string_value.clone() {
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
                self.update_global_specialized_function_value(name, &canonical_value_expression)?;
                self.ensure_global_property_descriptor_value(
                    name,
                    &canonical_value_expression,
                    true,
                );
            }
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        } else {
            let binding = self.module.ensure_implicit_global_binding(name);
            if !self.isolated_indirect_eval {
                self.module
                    .update_static_global_assignment_metadata(name, &canonical_value_expression);
                if let Some(text) = static_string_value {
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
                self.update_global_specialized_function_value(name, &canonical_value_expression)?;
                self.ensure_global_property_descriptor_value(
                    name,
                    &canonical_value_expression,
                    true,
                );
            }
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        }

        Ok(())
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
        if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
            self.local_kinds
                .insert(resolved_name, StaticValueKind::Number);
        } else if self.module.global_bindings.contains_key(name) {
            self.module
                .global_kinds
                .insert(name.to_string(), StaticValueKind::Number);
        } else {
            self.local_kinds
                .insert(name.to_string(), StaticValueKind::Number);
        }
    }
}
