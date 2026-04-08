use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_static_binding_call_result_with_context(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<(Expression, Option<String>)> {
        if let Some(result) = self.resolve_static_call_frame_binding_result_with_context(
            callee,
            arguments,
            current_function_name,
        ) {
            return Some(result);
        }

        let binding = self
            .resolve_function_binding_from_expression_with_context(callee, current_function_name)?;
        if let Some(outcome) = self.resolve_static_function_outcome_from_binding_with_context(
            &binding,
            arguments,
            current_function_name,
        ) {
            return match outcome {
                StaticEvalOutcome::Value(value) => Some((
                    value,
                    match binding {
                        LocalFunctionBinding::User(function_name) => Some(function_name),
                        LocalFunctionBinding::Builtin(_) => None,
                    },
                )),
                StaticEvalOutcome::Throw(_) => None,
            };
        }

        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        if user_function.has_lowered_pattern_parameters()
            || !self
                .user_function_parameter_iterator_consumption_indices(user_function)
                .is_empty()
        {
            return None;
        }
        if self
            .collect_user_function_assigned_nonlocal_bindings(user_function)
            .is_empty()
            && self
                .collect_user_function_call_effect_nonlocal_bindings(user_function)
                .is_empty()
        {
            let expanded_arguments = self.expand_call_arguments(arguments);
            if let Some((result, _)) = self
                .resolve_bound_snapshot_user_function_result_with_arguments(
                    &function_name,
                    &HashMap::new(),
                    &expanded_arguments,
                )
            {
                return Some((result, Some(function_name)));
            }
        }
        if !self.user_function_has_explicit_call_frame_inlineable_terminal_body(user_function) {
            return None;
        }

        let summary = user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        Some((
            self.substitute_user_function_argument_bindings(return_value, user_function, arguments),
            Some(function_name),
        ))
    }

    fn resolve_static_call_frame_binding_result_with_context(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<(Expression, Option<String>)> {
        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(name) if name == "call" || name == "apply")
        {
            let function_binding = self.resolve_function_binding_from_expression_with_context(
                object,
                current_function_name,
            )?;
            let LocalFunctionBinding::User(function_name) = &function_binding else {
                return None;
            };
            let user_function = self.user_function(function_name)?;
            if user_function.has_lowered_pattern_parameters()
                || !self
                    .user_function_parameter_iterator_consumption_indices(user_function)
                    .is_empty()
            {
                return None;
            }
            if !(user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| summary.effects.is_empty())
                || self
                    .user_function_has_explicit_call_frame_inlineable_terminal_body(user_function))
            {
                return None;
            }

            let expanded_arguments = self.expand_call_arguments(arguments);
            let raw_this_expression = expanded_arguments
                .first()
                .cloned()
                .unwrap_or(Expression::Undefined);
            let call_arguments =
                if matches!(property.as_ref(), Expression::String(name) if name == "call") {
                    expanded_arguments.into_iter().skip(1).collect::<Vec<_>>()
                } else {
                    let apply_expression = expanded_arguments
                        .get(1)
                        .cloned()
                        .unwrap_or(Expression::Undefined);
                    self.expand_apply_call_arguments_from_expression(&apply_expression)?
                        .into_iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => expression,
                        })
                        .collect::<Vec<_>>()
                };
            let this_binding =
                if self.should_box_sloppy_function_this(user_function, &raw_this_expression) {
                    Expression::This
                } else {
                    self.materialize_static_expression(&raw_this_expression)
                };
            let value = self.resolve_function_binding_static_return_expression_with_call_frame(
                &function_binding,
                &call_arguments,
                &this_binding,
            )?;
            return Some((value, Some(function_name.clone())));
        }

        if let Expression::Call {
            callee: bind_callee,
            arguments: bind_arguments,
        } = callee
            && let Expression::Member { object, property } = bind_callee.as_ref()
            && matches!(property.as_ref(), Expression::String(name) if name == "bind")
        {
            let function_binding = self.resolve_function_binding_from_expression_with_context(
                object,
                current_function_name,
            )?;
            let LocalFunctionBinding::User(function_name) = &function_binding else {
                return None;
            };
            let user_function = self.user_function(function_name)?;
            if user_function.has_lowered_pattern_parameters()
                || !self
                    .user_function_parameter_iterator_consumption_indices(user_function)
                    .is_empty()
            {
                return None;
            }
            if !(user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| summary.effects.is_empty())
                || self
                    .user_function_has_explicit_call_frame_inlineable_terminal_body(user_function))
            {
                return None;
            }

            let expanded_bind_arguments = self.expand_call_arguments(bind_arguments);
            let raw_this_expression = expanded_bind_arguments
                .first()
                .cloned()
                .unwrap_or(Expression::Undefined);
            let call_arguments = expanded_bind_arguments
                .into_iter()
                .skip(1)
                .chain(self.expand_call_arguments(arguments))
                .collect::<Vec<_>>();
            let this_binding =
                if self.should_box_sloppy_function_this(user_function, &raw_this_expression) {
                    Expression::This
                } else {
                    self.materialize_static_expression(&raw_this_expression)
                };
            let value = self.resolve_function_binding_static_return_expression_with_call_frame(
                &function_binding,
                &call_arguments,
                &this_binding,
            )?;
            return Some((value, Some(function_name.clone())));
        }

        None
    }
}
