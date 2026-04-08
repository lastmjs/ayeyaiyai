use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_simple_async_iterator_next_source(
        &self,
        iterator_value: &Expression,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>)> {
        let next_property = Expression::String("next".to_string());
        let next_outcome = if let Some(getter_binding) =
            self.resolve_member_getter_binding(iterator_value, &next_property)
        {
            self.resolve_static_function_outcome_from_binding_with_context(
                &getter_binding,
                &[],
                self.current_function_name(),
            )?
        } else {
            self.resolve_static_property_get_outcome(iterator_value, &next_property)?
        };
        self.validate_static_async_iterator_next_outcome(next_outcome)
    }

    pub(in crate::backend::direct_wasm) fn validate_static_async_iterator_next_outcome(
        &self,
        next_outcome: StaticEvalOutcome,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>)> {
        let current_function_name = self.current_function_name();
        let next_method_value = match next_outcome {
            StaticEvalOutcome::Throw(throw_value) => {
                return self.simple_generator_throw_step(throw_value);
            }
            StaticEvalOutcome::Value(next_method_value) => next_method_value,
        };
        if matches!(next_method_value, Expression::Undefined | Expression::Null) {
            return self.simple_generator_throw_step(StaticThrowValue::NamedError("TypeError"));
        }
        let Some(next_binding) = self.resolve_function_binding_from_expression_with_context(
            &next_method_value,
            current_function_name,
        ) else {
            return self.simple_generator_throw_step(StaticThrowValue::NamedError("TypeError"));
        };
        let next_result_outcome = self.resolve_static_function_outcome_from_binding_with_context(
            &next_binding,
            &[],
            current_function_name,
        )?;
        let awaited_result = match next_result_outcome {
            StaticEvalOutcome::Throw(throw_value) => {
                return self.simple_generator_throw_step(throw_value);
            }
            StaticEvalOutcome::Value(next_result) => {
                self.resolve_static_await_resolution_outcome(&next_result)?
            }
        };
        let awaited_result = match awaited_result {
            StaticEvalOutcome::Throw(throw_value) => {
                return self.simple_generator_throw_step(throw_value);
            }
            StaticEvalOutcome::Value(awaited_result) => awaited_result,
        };
        if !self.static_expression_is_object_like(&awaited_result) {
            return self.simple_generator_throw_step(StaticThrowValue::NamedError("TypeError"));
        }

        let done_property = Expression::String("done".to_string());
        let done_outcome =
            self.resolve_static_property_get_outcome(&awaited_result, &done_property)?;
        let done_value = match done_outcome {
            StaticEvalOutcome::Throw(throw_value) => {
                return self.simple_generator_throw_step(throw_value);
            }
            StaticEvalOutcome::Value(done_value) => done_value,
        };
        let done = self.resolve_static_boolean_expression(&done_value)?;

        let value_property = Expression::String("value".to_string());
        let value_outcome =
            self.resolve_static_property_get_outcome(&awaited_result, &value_property)?;
        let value = match value_outcome {
            StaticEvalOutcome::Throw(throw_value) => {
                return self.simple_generator_throw_step(throw_value);
            }
            StaticEvalOutcome::Value(value) => value,
        };

        if done {
            return Some((Vec::new(), Vec::new()));
        }

        Some((
            vec![SimpleGeneratorStep {
                effects: Vec::new(),
                outcome: SimpleGeneratorStepOutcome::Yield(value),
            }],
            Vec::new(),
        ))
    }
}
