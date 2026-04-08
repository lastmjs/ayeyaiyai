use super::*;

impl<'a> FunctionCompiler<'a> {
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
        let scopes = self.state.emission.lexical_scopes.with_scopes.clone();
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
                    if let Some(user_function) = self.user_function(&function_name) {
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
                    if let Some(user_function) = self.user_function(&function_name).cloned() {
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
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.push_global_get(binding.value_index);
            self.state.emission.output.instructions.push(0x05);
            if let Some(fallback_value) = fallback_value {
                self.emit_runtime_shadow_fallback_value(&fallback_value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.state.emission.output.instructions.push(0x0b);
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
}
