use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_async_yield_delegate_generator_plan(
        &self,
        expression: &Expression,
        completion_binding_name: &str,
    ) -> Option<AsyncYieldDelegateGeneratorPlan> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_function_binding_from_expression(callee)
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        if !matches!(user_function.kind, FunctionKind::AsyncGenerator)
            || user_function.has_parameter_defaults()
            || user_function.has_lowered_pattern_parameters()
            || !self
                .user_function_parameter_iterator_consumption_indices(user_function)
                .is_empty()
        {
            return None;
        }
        let function = self.resolve_registered_function_declaration(&function_name)?;
        let expanded_arguments = self.expand_call_arguments(arguments);
        let mut call_argument_values = expanded_arguments.clone();
        if call_argument_values.len() < user_function.params.len() {
            call_argument_values.resize(user_function.params.len(), Expression::Undefined);
        }
        let mut arguments_values = expanded_arguments;
        let raw_this_binding = self.resolve_generator_call_this_binding(callee);
        let analysis_this_binding =
            if self.should_box_sloppy_function_this(user_function, &raw_this_binding) {
                Expression::This
            } else {
                raw_this_binding
            };
        let substituted_body = self
            .substitute_simple_generator_statements_with_call_frame_bindings(
                &function.body,
                user_function,
                function.mapped_arguments && !function.strict,
                &mut call_argument_values,
                &mut arguments_values,
                &analysis_this_binding,
            )
            .unwrap_or_else(|| function.body.clone());

        let mut prefix_effects = Vec::new();
        let mut tail = Vec::new();
        let mut delegate_expression = None;
        let mut seen_delegate = false;
        for statement in substituted_body {
            if !seen_delegate {
                match statement {
                    Statement::YieldDelegate { value } => {
                        delegate_expression = Some(value);
                        seen_delegate = true;
                    }
                    statement if !Self::statement_contains_generator_yield(&statement) => {
                        prefix_effects.push(statement);
                    }
                    _ => return None,
                }
            } else {
                if Self::statement_contains_generator_yield(&statement) {
                    return None;
                }
                tail.push(statement);
            }
        }

        let delegate_expression = delegate_expression?;
        let (completion_value, completion_throw_value) = match tail.last() {
            Some(Statement::Return(value)) => {
                let value = value.clone();
                tail.pop();
                (value, None)
            }
            Some(Statement::Throw(value)) => {
                let value = value.clone();
                tail.pop();
                (Expression::Undefined, Some(value))
            }
            _ => (Expression::Undefined, None),
        };
        if tail
            .iter()
            .any(|statement| matches!(statement, Statement::Return(_) | Statement::Throw(_)))
        {
            return None;
        }

        let completion_placeholder = Expression::Identifier(completion_binding_name.to_string());
        let mut scope_bindings = user_function
            .scope_bindings
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        scope_bindings.sort();
        Some(AsyncYieldDelegateGeneratorPlan {
            function_name: user_function.name.clone(),
            prefix_effects,
            delegate_expression,
            completion_effects: tail
                .iter()
                .map(|statement| {
                    Self::substitute_sent_statement(statement, &completion_placeholder)
                })
                .collect(),
            completion_value: Self::substitute_sent_expression(
                &completion_value,
                &completion_placeholder,
            ),
            completion_throw_value: completion_throw_value
                .as_ref()
                .map(|value| Self::substitute_sent_expression(value, &completion_placeholder)),
            scope_bindings,
        })
    }
}
