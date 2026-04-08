use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_function_binding_static_return_expression_with_call_frame(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
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
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let arguments_binding = Expression::Array(
            arguments
                .iter()
                .cloned()
                .map(crate::ir::hir::ArrayElement::Expression)
                .collect(),
        );
        if let Some(summary) = user_function.inline_summary.as_ref()
            && self.user_function_has_explicit_call_frame_inlineable_terminal_body(user_function)
            && summary.effects.is_empty()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            return Some(self.substitute_user_function_call_frame_bindings(
                return_value,
                user_function,
                &call_arguments,
                this_binding,
                &arguments_binding,
            ));
        }

        if self
            .collect_user_function_assigned_nonlocal_bindings(user_function)
            .is_empty()
            && self
                .collect_user_function_call_effect_nonlocal_bindings(user_function)
                .is_empty()
            && let Some((result, _)) = self
                .resolve_bound_snapshot_user_function_result_with_arguments_and_this(
                    function_name,
                    &HashMap::new(),
                    arguments,
                    this_binding,
                )
        {
            return Some(result);
        }

        let function = self.resolve_registered_function_declaration(function_name)?;
        let (terminal_statement, effect_statements) = function.body.split_last()?;
        if !effect_statements
            .iter()
            .all(|statement| matches!(statement, Statement::Block { body } if body.is_empty()))
        {
            return None;
        }
        let Statement::Return(return_value) = terminal_statement else {
            return None;
        };
        Some(self.substitute_user_function_call_frame_bindings(
            return_value,
            user_function,
            &call_arguments,
            this_binding,
            &arguments_binding,
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_inline_call_from_returned_member(
        &self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let Expression::String(property_name) = property else {
            return None;
        };

        let (outer_callee, outer_arguments) = match object {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };

        let Expression::Identifier(outer_name) = outer_callee else {
            return None;
        };
        let outer_user_function = self.resolve_user_function_from_callee_name(outer_name)?;
        let returned_value = outer_user_function
            .returned_member_value_bindings
            .iter()
            .find(|binding| binding.property == *property_name)?
            .value
            .clone();
        let substituted_value = self.substitute_user_function_argument_bindings(
            &returned_value,
            outer_user_function,
            outer_arguments,
        );
        let Expression::Identifier(inner_name) = substituted_value else {
            return None;
        };
        let inner_user_function = self.user_function(&inner_name)?;
        let summary = inner_user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        let outer_substituted_return = self.substitute_user_function_argument_bindings(
            return_value,
            outer_user_function,
            outer_arguments,
        );

        Some(self.substitute_user_function_argument_bindings(
            &outer_substituted_return,
            inner_user_function,
            arguments,
        ))
    }
}
