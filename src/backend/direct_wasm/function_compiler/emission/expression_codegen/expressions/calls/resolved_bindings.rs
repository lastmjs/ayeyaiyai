use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_resolved_function_binding_call_expression(
        &mut self,
        source_expression: &Expression,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(function_binding) = self.resolve_function_binding_from_expression(callee) else {
            return Ok(false);
        };
        match function_binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) = self.user_function(&function_name).cloned() else {
                    return Ok(false);
                };
                if let Expression::Member { object, property } = callee {
                    let runtime_fallback = self
                        .promise_member_call_requires_runtime_fallback(object, property, arguments);
                    let materialized_this_expression = self.materialize_static_expression(object);
                    let materialized_call_arguments = arguments
                        .iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => {
                                self.materialize_static_expression(expression)
                            }
                        })
                        .collect::<Vec<_>>();
                    if let Some(capture_slots) =
                        self.resolve_member_function_capture_slots(object, property)
                    {
                        if runtime_fallback {
                            self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_without_static_snapshot(
                                &user_function,
                                arguments,
                                JS_UNDEFINED_TAG,
                                object,
                                &capture_slots,
                            )?;
                        } else {
                            self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                                &user_function,
                                arguments,
                                JS_UNDEFINED_TAG,
                                object,
                                &capture_slots,
                            )?;
                        }
                    } else {
                        if !runtime_fallback
                            && self.can_inline_user_function_call_with_explicit_call_frame(
                                &user_function,
                                &materialized_call_arguments,
                                &materialized_this_expression,
                            )
                        {
                            let result_local = self.allocate_temp_local();
                            if self.emit_inline_user_function_summary_with_explicit_call_frame(
                                &user_function,
                                &materialized_call_arguments,
                                &materialized_this_expression,
                                result_local,
                            )? {
                                self.push_local_get(result_local);
                                return Ok(true);
                            }
                        }
                        if runtime_fallback {
                            self.emit_user_function_call_with_new_target_and_this_expression_without_static_snapshot(
                                &user_function,
                                arguments,
                                JS_UNDEFINED_TAG,
                                object,
                            )?;
                        } else {
                            self.emit_user_function_call_with_function_this_binding(
                                &user_function,
                                arguments,
                                object,
                                None,
                            )?;
                        }
                        self.note_last_bound_user_function_source_expression(source_expression);
                    }
                } else if matches!(callee, Expression::SuperMember { .. }) {
                    self.emit_user_function_call_with_new_target_and_this_expression(
                        &user_function,
                        arguments,
                        JS_UNDEFINED_TAG,
                        &Expression::This,
                    )?;
                    self.note_last_bound_user_function_source_expression(source_expression);
                } else {
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
                }
                Ok(true)
            }
            LocalFunctionBinding::Builtin(function_name) => {
                if self.emit_builtin_call_for_callee(callee, &function_name, arguments, false)? {
                    return Ok(true);
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(true)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_member_function_binding_call_expression(
        &mut self,
        callee: &Expression,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(function_binding) = self.resolve_member_function_binding(object, property) else {
            return Ok(false);
        };
        match function_binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) = self.user_function(&function_name).cloned() else {
                    return Ok(false);
                };
                let runtime_fallback =
                    self.promise_member_call_requires_runtime_fallback(object, property, arguments);
                let materialized_this_expression = self.materialize_static_expression(object);
                let materialized_call_arguments = arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.materialize_static_expression(expression)
                        }
                    })
                    .collect::<Vec<_>>();
                if let Some(capture_slots) =
                    self.resolve_member_function_capture_slots(object, property)
                {
                    if runtime_fallback {
                        self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_without_static_snapshot(
                            &user_function,
                            arguments,
                            JS_UNDEFINED_TAG,
                            object,
                            &capture_slots,
                        )?;
                    } else {
                        self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                            &user_function,
                            arguments,
                            JS_UNDEFINED_TAG,
                            object,
                            &capture_slots,
                        )?;
                    }
                } else {
                    if !runtime_fallback
                        && self.can_inline_user_function_call_with_explicit_call_frame(
                            &user_function,
                            &materialized_call_arguments,
                            &materialized_this_expression,
                        )
                    {
                        let result_local = self.allocate_temp_local();
                        if self.emit_inline_user_function_summary_with_explicit_call_frame(
                            &user_function,
                            &materialized_call_arguments,
                            &materialized_this_expression,
                            result_local,
                        )? {
                            self.push_local_get(result_local);
                            return Ok(true);
                        }
                    }
                    if runtime_fallback {
                        self.emit_user_function_call_with_new_target_and_this_expression_without_static_snapshot(
                            &user_function,
                            arguments,
                            JS_UNDEFINED_TAG,
                            object,
                        )?;
                    } else {
                        self.emit_user_function_call_with_function_this_binding(
                            &user_function,
                            arguments,
                            object,
                            None,
                        )?;
                    }
                }
                Ok(true)
            }
            LocalFunctionBinding::Builtin(function_name) => {
                if self.emit_builtin_call_for_callee(callee, &function_name, arguments, false)? {
                    return Ok(true);
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(true)
            }
        }
    }
}
