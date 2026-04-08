use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_user_function_result(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        self.resolve_bound_snapshot_user_function_result_with_arguments_and_this(
            function_name,
            bindings,
            &[],
            &Expression::Undefined,
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_user_function_result_with_arguments(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        self.resolve_bound_snapshot_user_function_result_with_arguments_and_this(
            function_name,
            bindings,
            arguments,
            &Expression::Undefined,
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_user_function_result_with_arguments_and_this(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        let (outcome, local_bindings) = self
            .resolve_bound_snapshot_user_function_outcome_with_arguments_and_this(
                function_name,
                bindings,
                arguments,
                this_binding,
            )?;
        Some((
            match outcome {
                StaticEvalOutcome::Value(value) => value,
                StaticEvalOutcome::Throw(_) => Expression::Undefined,
            },
            local_bindings,
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_user_function_outcome_with_arguments_and_this(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> Option<(StaticEvalOutcome, HashMap<String, Expression>)> {
        let function = self.resolve_registered_function_declaration(function_name)?;
        let user_function = self.user_function(function_name)?;
        if user_function.has_parameter_defaults() {
            return None;
        }
        if user_function.has_lowered_pattern_parameters() {
            return None;
        }
        if !self
            .user_function_parameter_iterator_consumption_indices(user_function)
            .is_empty()
        {
            return None;
        }
        if function
            .body
            .iter()
            .any(Self::statement_contains_iterator_protocol_ops)
        {
            return None;
        }
        if !user_function.params.is_empty() && !user_function.extra_argument_indices.is_empty() {
            return None;
        }
        let mut local_bindings = bindings.clone();
        for (index, parameter_name) in user_function.params.iter().enumerate() {
            local_bindings.insert(
                parameter_name.clone(),
                arguments
                    .get(index)
                    .cloned()
                    .unwrap_or(Expression::Undefined),
            );
        }
        local_bindings.insert("this".to_string(), this_binding.clone());
        if let Expression::Identifier(this_name) = this_binding
            && !local_bindings.contains_key(this_name)
        {
            if let Some(value) = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(this_name)
                .cloned()
                .or_else(|| {
                    self.backend
                        .global_semantics
                        .values
                        .value_bindings
                        .get(this_name)
                        .cloned()
                })
            {
                local_bindings.insert(this_name.clone(), value);
            } else if let Some(object_binding) =
                self.resolve_object_binding_from_expression(this_binding)
            {
                local_bindings.insert(
                    this_name.clone(),
                    object_binding_to_expression(&object_binding),
                );
            }
        }
        let arguments_shadowed = user_function.lexical_this
            || user_function.body_declares_arguments_binding
            || user_function.params.iter().any(|param| {
                param == "arguments"
                    || scoped_binding_source_name(param)
                        .is_some_and(|source_name| source_name == "arguments")
            });
        if !arguments_shadowed {
            local_bindings.insert(
                "arguments".to_string(),
                Expression::Array(
                    arguments
                        .iter()
                        .cloned()
                        .map(ArrayElement::Expression)
                        .collect(),
                ),
            );
        }
        let result = self.execute_bound_snapshot_statements(
            &function.body,
            &mut local_bindings,
            Some(function_name),
        )?;
        Some((
            match result {
                BoundSnapshotControlFlow::None => StaticEvalOutcome::Value(Expression::Undefined),
                BoundSnapshotControlFlow::Return(value) => StaticEvalOutcome::Value(value),
                BoundSnapshotControlFlow::Throw(value) => {
                    StaticEvalOutcome::Throw(StaticThrowValue::Value(value))
                }
            },
            local_bindings,
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_function_outcome_with_arguments_and_this(
        &self,
        binding: &LocalFunctionBinding,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> Option<(StaticEvalOutcome, HashMap<String, Expression>)> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        self.resolve_bound_snapshot_user_function_outcome_with_arguments_and_this(
            function_name,
            bindings,
            arguments,
            this_binding,
        )
    }

    pub(in crate::backend::direct_wasm) fn apply_bound_snapshot_user_function_call_effects(
        &self,
        function_name: &str,
        arguments: &[Expression],
        this_binding: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let user_function = self.user_function(function_name)?;
        if user_function.is_async() || user_function.is_generator() {
            return None;
        }
        let evaluated_arguments = arguments
            .iter()
            .map(|argument| {
                self.evaluate_bound_snapshot_expression(argument, bindings, current_function_name)
            })
            .collect::<Option<Vec<_>>>()?;
        let (result, updated_bindings) = self
            .resolve_bound_snapshot_user_function_result_with_arguments_and_this(
                function_name,
                bindings,
                &evaluated_arguments,
                this_binding,
            )?;
        for (name, value) in updated_bindings {
            let source_name = scoped_binding_source_name(&name)
                .unwrap_or(&name)
                .to_string();
            if user_function.scope_bindings.contains(&source_name) {
                continue;
            }
            bindings.insert(source_name, value);
        }
        Some(result)
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_function_result_with_arguments(
        &self,
        binding: &LocalFunctionBinding,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        self.resolve_bound_snapshot_function_result_with_arguments_and_this(
            binding,
            bindings,
            arguments,
            &Expression::Undefined,
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_function_result_with_arguments_and_this(
        &self,
        binding: &LocalFunctionBinding,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        self.resolve_bound_snapshot_user_function_result_with_arguments_and_this(
            function_name,
            bindings,
            arguments,
            this_binding,
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_thenable_outcome(
        &self,
        binding: &LocalFunctionBinding,
        this_binding: &Expression,
        bindings: &mut HashMap<String, Expression>,
        _current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let (result, updated_bindings) = self
            .resolve_bound_snapshot_function_result_with_arguments_and_this(
                binding,
                bindings,
                &[
                    Expression::Identifier(SNAPSHOT_AWAIT_RESOLVE_BINDING.to_string()),
                    Expression::Identifier(SNAPSHOT_AWAIT_REJECT_BINDING.to_string()),
                ],
                this_binding,
            )?;
        *bindings = updated_bindings;
        let resolution = bindings
            .get(SNAPSHOT_AWAIT_RESOLUTION_VALUE)
            .cloned()
            .map(|value| self.sanitize_snapshot_await_marker_expression(&value));
        let rejection = bindings
            .get(SNAPSHOT_AWAIT_REJECTION_VALUE)
            .cloned()
            .map(|value| self.sanitize_snapshot_await_marker_expression(&value));
        for value in bindings.values_mut() {
            *value = self.sanitize_snapshot_await_marker_expression(value);
        }
        bindings.retain(|name, value| {
            name != SNAPSHOT_AWAIT_RESOLUTION_VALUE
                && name != SNAPSHOT_AWAIT_REJECTION_VALUE
                && name != SNAPSHOT_AWAIT_RESOLVE_BINDING
                && name != SNAPSHOT_AWAIT_REJECT_BINDING
                && !matches!(
                    value,
                    Expression::Identifier(marker)
                        if marker == SNAPSHOT_AWAIT_RESOLVE_BINDING
                            || marker == SNAPSHOT_AWAIT_REJECT_BINDING
                )
        });
        if let Some(resolution) = resolution {
            return self
                .resolve_static_await_resolution_outcome(&resolution)
                .or(Some(StaticEvalOutcome::Value(resolution)));
        }
        if let Some(rejection) = rejection {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(rejection)));
        }
        match result {
            Expression::Undefined => None,
            _ => self.resolve_static_await_resolution_outcome(&result),
        }
    }
}
