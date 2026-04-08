use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expand_apply_call_arguments_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<Vec<CallArgument>> {
        let materialized = self.materialize_static_expression(expression);
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            _ => {
                if let Some(array_binding) =
                    self.resolve_array_binding_from_expression(&materialized)
                {
                    return Some(
                        array_binding
                            .values
                            .into_iter()
                            .map(|value| {
                                CallArgument::Expression(value.unwrap_or(Expression::Undefined))
                            })
                            .collect(),
                    );
                }
                self.resolve_arguments_binding_from_expression(&materialized)
                    .map(|binding| {
                        binding
                            .values
                            .into_iter()
                            .map(CallArgument::Expression)
                            .collect()
                    })
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_expression_capture_slots(
        &self,
        expression: &Expression,
    ) -> Option<BTreeMap<String, String>> {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
            && let Some(capture_slots) = self.resolve_function_expression_capture_slots(&resolved)
        {
            return Some(capture_slots);
        }
        let Expression::Member { object, property } = expression else {
            return None;
        };
        self.resolve_member_function_capture_slots(object, property)
    }

    pub(in crate::backend::direct_wasm) fn should_box_sloppy_function_this(
        &self,
        user_function: &UserFunction,
        this_expression: &Expression,
    ) -> bool {
        if user_function.strict || user_function.lexical_this {
            return false;
        }
        matches!(
            self.infer_value_kind(this_expression),
            Some(
                StaticValueKind::Undefined
                    | StaticValueKind::Null
                    | StaticValueKind::Number
                    | StaticValueKind::BigInt
                    | StaticValueKind::String
                    | StaticValueKind::Bool
                    | StaticValueKind::Symbol
            )
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_function_this_binding(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        this_expression: &Expression,
        capture_slots: Option<&BTreeMap<String, String>>,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let simple_generator_call = Expression::Call {
            callee: Box::new(Expression::Identifier(user_function.name.clone())),
            arguments: expanded_arguments
                .iter()
                .cloned()
                .map(CallArgument::Expression)
                .collect(),
        };
        if user_function.is_generator()
            && self
                .resolve_simple_generator_source(&simple_generator_call)
                .is_some()
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        if capture_slots.is_none()
            && !self.should_box_sloppy_function_this(user_function, this_expression)
        {
            let materialized_this_expression = self.materialize_static_expression(this_expression);
            let materialized_call_arguments = expanded_arguments
                .iter()
                .map(|argument| self.materialize_static_expression(argument))
                .collect::<Vec<_>>();
            if self.can_inline_user_function_call_with_explicit_call_frame(
                user_function,
                &materialized_call_arguments,
                &materialized_this_expression,
            ) {
                let result_local = self.allocate_temp_local();
                if self.emit_inline_user_function_summary_with_explicit_call_frame(
                    user_function,
                    &expanded_arguments,
                    &materialized_this_expression,
                    result_local,
                )? {
                    self.push_local_get(result_local);
                    return Ok(());
                }
            }
        }
        if let Some(capture_slots) = capture_slots {
            return self
                .emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                    user_function,
                    arguments,
                    JS_UNDEFINED_TAG,
                    this_expression,
                    capture_slots,
                );
        }
        if self.should_box_sloppy_function_this(user_function, this_expression) {
            return self.emit_user_function_call_with_new_target_and_this(
                user_function,
                arguments,
                JS_UNDEFINED_TAG,
                JS_TYPEOF_OBJECT_TAG,
            );
        }
        self.emit_user_function_call_with_new_target_and_this_expression(
            user_function,
            arguments,
            JS_UNDEFINED_TAG,
            this_expression,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_function_binding_call_with_function_this_binding_from_argument_locals(
        &mut self,
        function_binding: &LocalFunctionBinding,
        argument_locals: &[u32],
        argument_count: usize,
        this_expression: &Expression,
    ) -> DirectResult<bool> {
        let LocalFunctionBinding::User(function_name) = function_binding else {
            return Ok(false);
        };
        let Some(user_function) = self.user_function(function_name).cloned() else {
            return Ok(false);
        };
        self.emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
            &user_function,
            argument_locals,
            argument_count,
            JS_UNDEFINED_TAG,
            this_expression,
        )?;
        Ok(true)
    }
}
