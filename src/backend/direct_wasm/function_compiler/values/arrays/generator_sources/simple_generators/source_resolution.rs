use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_array_prototype_simple_generator_source(
        &self,
        expression: &Expression,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>, Expression)> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_array_prototype_simple_generator_source(&materialized);
        }
        self.resolve_array_binding_from_expression(expression)?;

        let iterator_property = self.materialize_static_expression(&symbol_iterator_expression());
        let array_prototype = Expression::Member {
            object: Box::new(Expression::Identifier("Array".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_member_function_binding(&array_prototype, &iterator_property)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        if !user_function.is_generator()
            || !user_function.params.is_empty()
            || user_function.has_parameter_defaults()
            || !user_function.extra_argument_indices.is_empty()
        {
            return None;
        }

        let function = self.resolve_registered_function_declaration(&function_name)?;
        let mut call_argument_values = user_function
            .params
            .iter()
            .map(|_| Expression::Undefined)
            .collect::<Vec<_>>();
        let mut arguments_values = Vec::new();
        let analysis_this_binding = if self
            .runtime_array_length_local_for_expression(expression)
            .is_some()
        {
            let array_binding = self.resolve_array_binding_from_expression(expression)?;
            Expression::Array(
                array_binding
                    .values
                    .into_iter()
                    .map(|value| {
                        crate::ir::hir::ArrayElement::Expression(
                            value.unwrap_or(Expression::Undefined),
                        )
                    })
                    .collect(),
            )
        } else {
            expression.clone()
        };
        let substituted_body = self
            .substitute_simple_generator_statements_with_call_frame_bindings(
                &function.body,
                user_function,
                function.mapped_arguments && !function.strict,
                &mut call_argument_values,
                &mut arguments_values,
                &analysis_this_binding,
            )?;
        let (substituted_body, completion_value) =
            self.split_simple_generator_completion(substituted_body)?;
        let mut steps = Vec::new();
        let mut effects = Vec::new();
        self.analyze_simple_generator_statements(
            &substituted_body,
            matches!(user_function.kind, FunctionKind::AsyncGenerator),
            &mut steps,
            &mut effects,
        )?;
        Some((steps, effects, completion_value))
    }

    pub(in crate::backend::direct_wasm) fn resolve_simple_generator_source(
        &self,
        expression: &Expression,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>, Expression)> {
        if let Expression::Call { callee, arguments } = expression
            && let Some(LocalFunctionBinding::User(function_name)) =
                self.resolve_function_binding_from_expression(callee)
            && let Some(user_function) = self.user_function(&function_name)
        {
            if !user_function.is_generator()
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
                )?;

            let (substituted_body, completion_value) =
                self.split_simple_generator_completion(substituted_body)?;
            let mut steps = Vec::new();
            let mut effects = Vec::new();
            self.analyze_simple_generator_statements(
                &substituted_body,
                matches!(user_function.kind, FunctionKind::AsyncGenerator),
                &mut steps,
                &mut effects,
            )?;
            return Some((steps, effects, completion_value));
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_simple_generator_source(&materialized);
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn analyze_effectful_iterator_source_call(
        &self,
        expression: &Expression,
    ) -> Option<(String, Expression, Vec<Statement>)> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.analyze_effectful_iterator_source_call(&materialized);
        }

        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        if user_function.is_async()
            || user_function.is_generator()
            || user_function.has_parameter_defaults()
            || !user_function.extra_argument_indices.is_empty()
        {
            return None;
        }

        let function = self.resolve_registered_function_declaration(&function_name)?;
        let (terminal_statement, effect_statements) = function.body.split_last()?;
        let mut substituted_effects = Vec::new();
        for statement in effect_statements {
            match statement {
                Statement::Assign { name, value } => {
                    let substituted = self.substitute_user_function_argument_bindings(
                        value,
                        user_function,
                        arguments,
                    );
                    if expression_mentions_call_frame_state(&substituted) {
                        return None;
                    }
                    substituted_effects.push(Statement::Assign {
                        name: name.clone(),
                        value: substituted,
                    });
                }
                Statement::Expression(Expression::Update { name, op, prefix }) => {
                    substituted_effects.push(Statement::Expression(Expression::Update {
                        name: name.clone(),
                        op: *op,
                        prefix: *prefix,
                    }));
                }
                Statement::Expression(effect_expression) => {
                    let substituted = self.substitute_user_function_argument_bindings(
                        effect_expression,
                        user_function,
                        arguments,
                    );
                    if expression_mentions_call_frame_state(&substituted) {
                        return None;
                    }
                    substituted_effects.push(Statement::Expression(substituted));
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return None,
            }
        }

        let Statement::Return(return_value) = terminal_statement else {
            return None;
        };
        let returned_expression =
            self.substitute_user_function_argument_bindings(return_value, user_function, arguments);
        if expression_mentions_call_frame_state(&returned_expression)
            || static_expression_matches(&returned_expression, expression)
        {
            return None;
        }

        Some((function_name, returned_expression, substituted_effects))
    }
}
