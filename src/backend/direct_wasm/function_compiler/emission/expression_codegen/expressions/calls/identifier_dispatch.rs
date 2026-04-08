use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_identifier_call_expression(
        &mut self,
        source_expression: &Expression,
        callee: &Expression,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        let resolved_local_name = self
            .resolve_current_local_binding(name)
            .map(|(resolved_name, _)| resolved_name);
        if resolved_local_name.is_some()
            || self.resolve_eval_local_function_hidden_name(name).is_some()
        {
            let binding_name = resolved_local_name.as_deref().unwrap_or(name);
            if let Some(function_name) = self
                .state
                .speculation
                .static_semantics
                .local_function_binding(binding_name)
                .cloned()
            {
                match function_name {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) = self.user_function(&function_name).cloned() {
                            if let Some(capture_slots) =
                                self.resolve_function_expression_capture_slots(callee)
                            {
                                self.emit_user_function_call_with_function_this_binding(
                                    &user_function,
                                    arguments,
                                    &Expression::Undefined,
                                    Some(&capture_slots),
                                )?;
                            } else {
                                self.emit_user_function_call(&user_function, arguments)?;
                            }
                            return Ok(());
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        if self.emit_builtin_call_for_callee(
                            callee,
                            &function_name,
                            arguments,
                            false,
                        )? {
                            return Ok(());
                        }
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        return Ok(());
                    }
                }
            }
            if let Some(value) = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(binding_name)
                .cloned()
                && let Some(function_binding) =
                    self.resolve_function_binding_from_expression(&value)
            {
                match function_binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) = self.user_function(&function_name).cloned() {
                            if let Some(capture_slots) =
                                self.resolve_function_expression_capture_slots(callee)
                            {
                                self.emit_user_function_call_with_function_this_binding(
                                    &user_function,
                                    arguments,
                                    &Expression::Undefined,
                                    Some(&capture_slots),
                                )?;
                            } else {
                                self.emit_user_function_call(&user_function, arguments)?;
                            }
                            self.note_last_bound_user_function_source_expression(source_expression);
                            return Ok(());
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        if self.emit_builtin_call_for_callee(
                            callee,
                            &function_name,
                            arguments,
                            false,
                        )? {
                            return Ok(());
                        }
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        return Ok(());
                    }
                }
            }

            if self.emit_dynamic_user_function_call(callee, arguments)? {
                return Ok(());
            }
            self.emit_ignored_call_arguments(arguments)?;
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }

        if name == "__ayyAssertThrows" && self.emit_assert_throws_call(arguments)? {
            return Ok(());
        }
        if matches!(
            name,
            "__assert" | "__assertSameValue" | "__assertNotSameValue"
        ) && self.emit_builtin_call(name, arguments)?
        {
            return Ok(());
        }

        if let Some(function_binding) = self
            .backend
            .global_semantics
            .functions
            .function_binding(name)
            .cloned()
        {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name).cloned() {
                        if let Some(capture_slots) =
                            self.resolve_function_expression_capture_slots(callee)
                        {
                            self.emit_user_function_call_with_function_this_binding(
                                &user_function,
                                arguments,
                                &Expression::Undefined,
                                Some(&capture_slots),
                            )?;
                        } else {
                            self.emit_user_function_call(&user_function, arguments)?;
                        }
                        return Ok(());
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    if self.emit_builtin_call_for_callee(
                        callee,
                        &function_name,
                        arguments,
                        false,
                    )? {
                        return Ok(());
                    }
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
            }
        }
        if let Some(value) = self
            .backend
            .global_semantics
            .values
            .value_bindings
            .get(name)
            .cloned()
            && let Some(function_binding) = self.resolve_function_binding_from_expression(&value)
        {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name).cloned() {
                        if let Some(capture_slots) =
                            self.resolve_function_expression_capture_slots(callee)
                        {
                            self.emit_user_function_call_with_function_this_binding(
                                &user_function,
                                arguments,
                                &Expression::Undefined,
                                Some(&capture_slots),
                            )?;
                        } else {
                            self.emit_user_function_call(&user_function, arguments)?;
                        }
                        return Ok(());
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    if self.emit_builtin_call_for_callee(
                        callee,
                        &function_name,
                        arguments,
                        false,
                    )? {
                        return Ok(());
                    }
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
            }
        }
        if name == "compareArray" && self.emit_compare_array_call(arguments)? {
            return Ok(());
        }
        if name == "verifyProperty" && self.emit_verify_property_call(arguments)? {
            return Ok(());
        }
        if is_internal_user_function_identifier(name)
            && let Some(user_function) = self.user_function(name).cloned()
        {
            if let Some(capture_slots) = self.resolve_function_expression_capture_slots(callee) {
                self.emit_user_function_call_with_function_this_binding(
                    &user_function,
                    arguments,
                    &Expression::Undefined,
                    Some(&capture_slots),
                )?;
            } else {
                self.emit_user_function_call(&user_function, arguments)?;
            }
            return Ok(());
        }
        if self.emit_builtin_call_for_callee(callee, name, arguments, false)? {
            return Ok(());
        }

        if self.emit_dynamic_user_function_call(callee, arguments)? {
            return Ok(());
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }
}
