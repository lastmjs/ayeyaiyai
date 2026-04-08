use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn inline_safe_argument_expression(
        &self,
        expression: &Expression,
    ) -> bool {
        let materialized = self.materialize_static_expression(expression);
        matches!(
            materialized,
            Expression::Number(_)
                | Expression::BigInt(_)
                | Expression::String(_)
                | Expression::Bool(_)
                | Expression::Null
                | Expression::Undefined
                | Expression::This
                | Expression::Array(_)
        ) || matches!(materialized, Expression::Object(ref entries)
            if entries.iter().all(|entry| matches!(entry, ObjectEntry::Data { .. })))
            || matches!(
                materialized,
                Expression::Member { ref object, ref property }
                    if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
                        && !matches!(object.as_ref(), Expression::SuperMember { .. })
            )
            || self
                .resolve_object_binding_from_expression(expression)
                .is_some()
            || self
                .resolve_array_binding_from_expression(expression)
                .is_some()
            || self
                .resolve_function_binding_from_expression(expression)
                .is_some()
            || self
                .resolve_user_function_from_expression(expression)
                .is_some()
            || self
                .resolve_symbol_identity_expression(&materialized)
                .is_some()
            || self
                .resolve_symbol_identity_expression(expression)
                .is_some()
    }

    pub(in crate::backend::direct_wasm) fn can_inline_user_function_call(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> bool {
        !self.current_function_contains_try_statement()
            && arguments.iter().all(|argument| {
                let materialized = self.materialize_static_expression(argument);
                static_expression_matches(&materialized, argument)
                    && self.inline_safe_argument_expression(argument)
            })
            && !arguments
                .iter()
                .any(|argument| self.inline_argument_mentions_shadowed_implicit_global(argument))
            && !user_function.is_async()
            && !user_function.is_generator()
            && !self
                .backend
                .function_registry
                .analysis
                .user_function_capture_bindings
                .contains_key(&user_function.name)
            && !self.user_function_references_captured_user_function(user_function)
            && user_function.extra_argument_indices.is_empty()
            && !user_function.has_parameter_defaults()
            && !user_function.has_lowered_pattern_parameters()
            && (user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| {
                    !inline_summary_mentions_assertion_builtin(summary)
                        && (user_function.lexical_this
                            || !inline_summary_mentions_call_frame_state(summary))
                })
                || self.user_function_has_inlineable_terminal_body(user_function))
    }

    pub(in crate::backend::direct_wasm) fn can_inline_user_function_call_with_explicit_call_frame(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_expression: &Expression,
    ) -> bool {
        !self.current_function_contains_try_statement()
            && self.inline_safe_argument_expression(this_expression)
            && !self.inline_argument_mentions_shadowed_implicit_global(this_expression)
            && arguments
                .iter()
                .all(|argument| self.inline_safe_argument_expression(argument))
            && !arguments
                .iter()
                .any(|argument| self.inline_argument_mentions_shadowed_implicit_global(argument))
            && !user_function.is_async()
            && !user_function.is_generator()
            && user_function.extra_argument_indices.is_empty()
            && !user_function.has_parameter_defaults()
            && !user_function.has_lowered_pattern_parameters()
            && (user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| {
                    !inline_summary_mentions_assertion_builtin(summary)
                        && !inline_summary_mentions_unsupported_explicit_call_frame_state(summary)
                })
                || self
                    .user_function_has_explicit_call_frame_inlineable_terminal_body(user_function))
    }
}
