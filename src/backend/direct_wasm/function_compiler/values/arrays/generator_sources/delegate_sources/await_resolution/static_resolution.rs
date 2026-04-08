use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_await_resolution_outcome(
        &self,
        resolution: &Expression,
    ) -> Option<StaticEvalOutcome> {
        let current_function_name = self.current_function_name();
        if let Expression::Call { callee, arguments } = resolution {
            if let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                callee,
                current_function_name,
            ) {
                match &binding {
                    LocalFunctionBinding::Builtin(name) if name == "Promise.resolve" => {
                        let settled_argument = arguments.first().map(|argument| match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => {
                                self.materialize_static_expression(expression)
                            }
                        });
                        return Some(match settled_argument {
                            Some(argument) => self
                                .resolve_static_await_resolution_outcome(&argument)
                                .unwrap_or(StaticEvalOutcome::Value(argument)),
                            None => StaticEvalOutcome::Value(Expression::Undefined),
                        });
                    }
                    LocalFunctionBinding::Builtin(name) if name == "Promise.reject" => {
                        let settled_argument = arguments.first().map(|argument| match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => {
                                self.materialize_static_expression(expression)
                            }
                        });
                        return Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(
                            settled_argument.unwrap_or(Expression::Undefined),
                        )));
                    }
                    LocalFunctionBinding::User(_) => {
                        let call_arguments = self.expand_call_arguments(arguments);
                        let this_binding = match callee.as_ref() {
                            Expression::Member { object, .. } => {
                                self.materialize_static_expression(object)
                            }
                            Expression::SuperMember { .. } => Expression::This,
                            _ => Expression::Undefined,
                        };
                        if let Some(value) = self
                            .resolve_function_binding_static_return_expression_with_call_frame(
                                &binding,
                                &call_arguments,
                                &this_binding,
                            )
                        {
                            return self
                                .resolve_static_await_resolution_outcome(&value)
                                .or(Some(StaticEvalOutcome::Value(value)));
                        }
                    }
                    _ => {}
                }
                if let Some(outcome) = self
                    .resolve_static_function_outcome_from_binding_with_context(
                        &binding,
                        arguments,
                        current_function_name,
                    )
                {
                    return Some(match outcome {
                        StaticEvalOutcome::Value(value) => self
                            .resolve_static_await_resolution_outcome(&value)
                            .unwrap_or(StaticEvalOutcome::Value(value)),
                        StaticEvalOutcome::Throw(throw_value) => {
                            StaticEvalOutcome::Throw(throw_value)
                        }
                    });
                }
            }
            if let Expression::Member { object, property } = callee.as_ref()
                && matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                && let Expression::String(property_name) = property.as_ref()
            {
                let settled_argument = arguments.first().map(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.materialize_static_expression(expression)
                    }
                });
                match property_name.as_str() {
                    "resolve" => {
                        return Some(match settled_argument {
                            Some(argument) => self
                                .resolve_static_await_resolution_outcome(&argument)
                                .unwrap_or(StaticEvalOutcome::Value(argument)),
                            None => StaticEvalOutcome::Value(Expression::Undefined),
                        });
                    }
                    "reject" => {
                        return Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(
                            settled_argument.unwrap_or(Expression::Undefined),
                        )));
                    }
                    _ => {}
                }
            }
            if let Some(result) = self.resolve_static_call_result_expression(callee, arguments) {
                return self
                    .resolve_static_await_resolution_outcome(&result)
                    .or(Some(StaticEvalOutcome::Value(result)));
            }
        }
        let materialized = self.materialize_static_expression(resolution);
        if !static_expression_matches(&materialized, resolution) {
            return self.resolve_static_await_resolution_outcome(&materialized);
        }
        if let Expression::Call { callee, arguments } = &materialized
            && let Expression::Member { object, property } = callee.as_ref()
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
            && let Expression::String(property_name) = property.as_ref()
        {
            let settled_argument = arguments.first().map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.materialize_static_expression(expression)
                }
            });
            match property_name.as_str() {
                "resolve" => {
                    return Some(match settled_argument {
                        Some(argument) => self
                            .resolve_static_await_resolution_outcome(&argument)
                            .unwrap_or(StaticEvalOutcome::Value(argument)),
                        None => StaticEvalOutcome::Value(Expression::Undefined),
                    });
                }
                "reject" => {
                    return Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(
                        settled_argument.unwrap_or(Expression::Undefined),
                    )));
                }
                _ => {}
            }
        }
        if self
            .resolve_static_primitive_expression_with_context(&materialized, current_function_name)
            .is_some()
        {
            return Some(StaticEvalOutcome::Value(materialized));
        }
        if !self.static_expression_is_object_like(&materialized) {
            return Some(StaticEvalOutcome::Value(materialized));
        }

        let then_property = Expression::String("then".to_string());
        let mut snapshot_bindings = HashMap::new();
        let then_outcome = match &materialized {
            Expression::Object(entries) => self
                .resolve_bound_snapshot_object_member_outcome(
                    entries,
                    &then_property,
                    &mut snapshot_bindings,
                    current_function_name,
                )
                .or_else(|| {
                    self.resolve_static_property_get_outcome(&materialized, &then_property)
                })?,
            _ => self.resolve_static_property_get_outcome(&materialized, &then_property)?,
        };
        match then_outcome {
            StaticEvalOutcome::Throw(throw_value) => Some(StaticEvalOutcome::Throw(throw_value)),
            StaticEvalOutcome::Value(then_value) => {
                if matches!(then_value, Expression::Undefined | Expression::Null) {
                    return Some(StaticEvalOutcome::Value(materialized));
                }
                let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                    &then_value,
                    current_function_name,
                ) else {
                    return Some(StaticEvalOutcome::Value(materialized));
                };
                if let Some(outcome) = self.resolve_bound_snapshot_thenable_outcome(
                    &binding,
                    &materialized,
                    &mut snapshot_bindings,
                    current_function_name,
                ) {
                    return Some(outcome);
                }
                match self.resolve_static_function_outcome_from_binding_with_context(
                    &binding,
                    &[],
                    current_function_name,
                )? {
                    StaticEvalOutcome::Throw(throw_value) => {
                        Some(StaticEvalOutcome::Throw(throw_value))
                    }
                    StaticEvalOutcome::Value(_) => None,
                }
            }
        }
    }
}
