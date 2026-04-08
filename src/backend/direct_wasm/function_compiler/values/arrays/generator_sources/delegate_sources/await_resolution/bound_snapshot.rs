use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_await_resolution_outcome(
        &self,
        resolution: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let materialized = self
            .evaluate_bound_snapshot_expression(resolution, bindings, current_function_name)
            .unwrap_or_else(|| self.materialize_static_expression(resolution));
        if let Expression::Call { callee, arguments } = &materialized
            && let Expression::Member { object, property } = callee.as_ref()
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
            && let Expression::String(property_name) = property.as_ref()
        {
            let settled_argument = arguments.first().map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => self
                    .evaluate_bound_snapshot_expression(expression, bindings, current_function_name)
                    .unwrap_or_else(|| self.materialize_static_expression(expression)),
            });
            match property_name.as_str() {
                "resolve" => {
                    return Some(match settled_argument {
                        Some(argument) => self
                            .resolve_bound_snapshot_await_resolution_outcome(
                                &argument,
                                bindings,
                                current_function_name,
                            )
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
        let then_value = match &materialized {
            Expression::Object(entries) => self
                .resolve_bound_snapshot_object_member_outcome(
                    entries,
                    &then_property,
                    bindings,
                    current_function_name,
                )
                .unwrap_or(StaticEvalOutcome::Value(Expression::Undefined)),
            _ => self
                .resolve_static_property_get_outcome(&materialized, &then_property)
                .unwrap_or(StaticEvalOutcome::Value(Expression::Undefined)),
        };
        let then_value = match then_value {
            StaticEvalOutcome::Value(value) => value,
            StaticEvalOutcome::Throw(throw_value) => {
                return Some(StaticEvalOutcome::Throw(throw_value));
            }
        };
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
            bindings,
            current_function_name,
        ) {
            return Some(outcome);
        }
        match self.resolve_static_function_outcome_from_binding_with_context(
            &binding,
            &[],
            current_function_name,
        )? {
            StaticEvalOutcome::Throw(throw_value) => Some(StaticEvalOutcome::Throw(throw_value)),
            StaticEvalOutcome::Value(_) => None,
        }
    }
}
