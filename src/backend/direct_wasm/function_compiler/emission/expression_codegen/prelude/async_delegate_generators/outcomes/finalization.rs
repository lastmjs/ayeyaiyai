use super::*;

impl<'a> FunctionCompiler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::backend::direct_wasm) fn finalize_async_yield_delegate_generator_outcome(
        &mut self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        property_name: &str,
        step_result_name: &str,
        promise_done_name: &str,
        promise_value_name: &str,
        delegate_completion_expression: &Expression,
        binding_name: &str,
        current_static_index: Option<usize>,
        delegate_snapshot_bindings: Option<HashMap<String, Expression>>,
        scoped_snapshot_names: &[String],
        static_step_result_has_accessor_properties: bool,
    ) -> DirectResult<Option<StaticEvalOutcome>> {
        let step_result_expression = Expression::Identifier(step_result_name.to_string());
        let mut returned_done_expression = Expression::Identifier(promise_done_name.to_string());
        let mut returned_value_expression = Expression::Identifier(promise_value_name.to_string());
        if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_ref() {
            if let Some(done_expression) = snapshot_bindings.get(promise_done_name) {
                returned_done_expression = done_expression.clone();
            }
            if let Some(value_expression) = snapshot_bindings.get(promise_value_name) {
                returned_value_expression = value_expression.clone();
            }
        }
        if let Some(resolved_done) = self
            .resolve_bound_alias_expression(&Expression::Identifier(promise_done_name.to_string()))
            .filter(|resolved| {
                !static_expression_matches(
                    resolved,
                    &Expression::Identifier(promise_done_name.to_string()),
                )
            })
        {
            returned_done_expression = resolved_done;
        }
        if let Some(resolved_value) = self
            .resolve_bound_alias_expression(&Expression::Identifier(promise_value_name.to_string()))
            .filter(|resolved| {
                !static_expression_matches(
                    resolved,
                    &Expression::Identifier(promise_value_name.to_string()),
                )
            })
        {
            returned_value_expression = resolved_value;
        }
        if !static_step_result_has_accessor_properties
            && let Some(step_result_binding) =
                self.resolve_object_binding_from_expression(&step_result_expression)
        {
            let done_property = self.resolve_object_binding_property_value(
                &step_result_binding,
                &Expression::String("done".to_string()),
            );
            let value_property = self.resolve_object_binding_property_value(
                &step_result_binding,
                &Expression::String("value".to_string()),
            );
            if let Some(done_property) = done_property
                && let Some(done) = self.resolve_static_boolean_expression(&done_property)
            {
                returned_done_expression = Expression::Bool(done);
                if done {
                    match property_name {
                        "return" => {
                            returned_value_expression = delegate_completion_expression.clone();
                        }
                        "next" | "throw" => {
                            returned_value_expression = plan.completion_value.clone();
                        }
                        _ => {}
                    }
                } else if let Some(value_property) = value_property {
                    returned_value_expression = value_property;
                }
            }
        }
        let materialized_returned_done_expression =
            self.materialize_static_expression(&returned_done_expression);
        if !static_expression_matches(
            &materialized_returned_done_expression,
            &returned_done_expression,
        ) {
            returned_done_expression = materialized_returned_done_expression;
        }
        let materialized_returned_value_expression =
            self.materialize_static_expression(&returned_value_expression);
        if !static_expression_matches(
            &materialized_returned_value_expression,
            &returned_value_expression,
        ) {
            returned_value_expression = materialized_returned_value_expression;
        }

        let awaited_yield_outcome = match self
            .resolve_static_boolean_expression(&returned_done_expression)
        {
            Some(false) => self.resolve_static_await_resolution_outcome(&returned_value_expression),
            _ => None,
        };
        if let Some(StaticEvalOutcome::Throw(throw_value)) = awaited_yield_outcome {
            self.persist_async_yield_delegate_generator_snapshot_state(
                binding_name,
                Some(2),
                delegate_snapshot_bindings,
            );
            self.sync_persisted_async_yield_delegate_generator_snapshot_state(binding_name)?;
            self.pop_async_delegate_snapshot_scope_bindings(scoped_snapshot_names);
            return Ok(Some(StaticEvalOutcome::Throw(throw_value)));
        }
        if let Some(StaticEvalOutcome::Value(awaited_value)) = awaited_yield_outcome {
            returned_value_expression = awaited_value;
        }

        let next_static_index = match current_static_index {
            Some(index) if index >= 2 => Some(2),
            Some(_) => match self.resolve_static_boolean_expression(&returned_done_expression) {
                Some(true) => Some(2),
                Some(false) => Some(1),
                None => None,
            },
            None => None,
        };
        self.persist_async_yield_delegate_generator_snapshot_state(
            binding_name,
            next_static_index,
            delegate_snapshot_bindings,
        );
        self.sync_persisted_async_yield_delegate_generator_snapshot_state(binding_name)?;
        let outcome = Some(StaticEvalOutcome::Value(Expression::Object(vec![
            ObjectEntry::Data {
                key: Expression::String("done".to_string()),
                value: returned_done_expression,
            },
            ObjectEntry::Data {
                key: Expression::String("value".to_string()),
                value: returned_value_expression,
            },
        ])));
        self.pop_async_delegate_snapshot_scope_bindings(scoped_snapshot_names);
        Ok(outcome)
    }
}
