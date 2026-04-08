use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn consume_prepared_async_yield_delegate_generator_promise_outcome(
        &mut self,
        prepared: PreparedAsyncDelegateConsumption,
    ) -> DirectResult<Option<StaticEvalOutcome>> {
        let PreparedAsyncDelegateConsumption {
            binding_name,
            current_static_index,
            index_local,
            property_name,
            plan,
            delegate_iterator_name,
            delegate_next_name,
            delegate_completion_name,
            delegate_iterator_expression,
            delegate_completion_expression,
            mut delegate_snapshot_bindings,
            scoped_snapshot_names,
            snapshot_current_argument,
            step_result_name,
            promise_value_name,
            promise_done_name,
        } = prepared;

        let delegate_next_expression = Expression::Identifier(delegate_next_name.clone());
        let static_delegate_next_expression = delegate_snapshot_bindings
            .as_ref()
            .and_then(|snapshot_bindings| snapshot_bindings.get(delegate_next_name.as_str()))
            .cloned()
            .unwrap_or(delegate_next_expression.clone());
        let delegate_next_binding = self
            .resolve_function_binding_from_expression(&static_delegate_next_expression)
            .or_else(|| self.resolve_function_binding_from_expression(&delegate_next_expression));
        let step_result_expression = Expression::Identifier(step_result_name.clone());
        let done_property = Expression::String("done".to_string());
        let value_property = Expression::String("value".to_string());
        let static_step_result_has_accessor_properties = self
            .resolve_member_getter_binding(&step_result_expression, &done_property)
            .is_some()
            || self
                .resolve_member_getter_binding(&step_result_expression, &value_property)
                .is_some();
        let (_static_step_result_expression, static_step_result_has_accessor_properties) =
            if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_mut() {
                let static_call_outcome =
                    if let Some(function_binding) = delegate_next_binding.as_ref() {
                        self.resolve_bound_snapshot_function_outcome_with_arguments_and_this(
                            function_binding,
                            snapshot_bindings,
                            &[snapshot_current_argument.clone()],
                            &delegate_iterator_expression,
                        )
                    } else {
                        None
                    };
                if let Some((static_call_outcome, updated_bindings)) = static_call_outcome {
                    *snapshot_bindings = updated_bindings;
                    match static_call_outcome {
                        StaticEvalOutcome::Value(static_result) => {
                            snapshot_bindings
                                .insert(step_result_name.clone(), static_result.clone());
                            (static_result, static_step_result_has_accessor_properties)
                        }
                        StaticEvalOutcome::Throw(throw_value) => {
                            self.persist_async_yield_delegate_generator_snapshot_state(
                                &binding_name,
                                Some(2),
                                Some(delegate_snapshot_bindings.clone().unwrap()),
                            );
                            self.sync_persisted_async_yield_delegate_generator_snapshot_state(
                                &binding_name,
                            )?;
                            self.pop_async_delegate_snapshot_scope_bindings(&scoped_snapshot_names);
                            return Ok(Some(StaticEvalOutcome::Throw(throw_value)));
                        }
                    }
                } else {
                    (
                        Expression::Identifier(step_result_name.clone()),
                        static_step_result_has_accessor_properties,
                    )
                }
            } else {
                (
                    Expression::Identifier(step_result_name.clone()),
                    static_step_result_has_accessor_properties,
                )
            };
        let runtime_step_result_expression = Expression::Identifier(step_result_name.clone());
        if let Some(done_expression) = delegate_snapshot_bindings
            .as_ref()
            .and_then(|snapshot_bindings| snapshot_bindings.get(&promise_done_name))
            .cloned()
        {
            self.emit_statement(&Statement::Assign {
                name: promise_done_name.clone(),
                value: done_expression,
            })?;
        } else if !self.emit_async_yield_delegate_step_result_getter_assignment(
            &step_result_name,
            &runtime_step_result_expression,
            &promise_done_name,
            "done",
        )? {
            self.emit_statement(&Statement::Assign {
                name: promise_done_name.clone(),
                value: Expression::Member {
                    object: Box::new(runtime_step_result_expression.clone()),
                    property: Box::new(Expression::String("done".to_string())),
                },
            })?;
        }
        let static_done = self
            .resolve_static_boolean_expression(&Expression::Identifier(promise_done_name.clone()));
        match static_done {
            Some(true) => self.emit_async_yield_delegate_done_branch(
                &plan,
                delegate_snapshot_bindings.as_ref(),
                &runtime_step_result_expression,
                &step_result_name,
                &delegate_completion_name,
                &delegate_completion_expression,
                &promise_value_name,
                &promise_done_name,
                property_name.as_str(),
                index_local,
            )?,
            Some(false) => self.emit_async_yield_delegate_not_done_branch(
                delegate_snapshot_bindings.as_ref(),
                &runtime_step_result_expression,
                &step_result_name,
                &promise_value_name,
                &promise_done_name,
            )?,
            None => {
                self.emit_numeric_expression(&Expression::Identifier(promise_done_name.clone()))?;
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.emit_async_yield_delegate_done_branch(
                    &plan,
                    delegate_snapshot_bindings.as_ref(),
                    &runtime_step_result_expression,
                    &step_result_name,
                    &delegate_completion_name,
                    &delegate_completion_expression,
                    &promise_value_name,
                    &promise_done_name,
                    property_name.as_str(),
                    index_local,
                )?;
                self.state.emission.output.instructions.push(0x05);
                self.emit_async_yield_delegate_not_done_branch(
                    delegate_snapshot_bindings.as_ref(),
                    &runtime_step_result_expression,
                    &step_result_name,
                    &promise_value_name,
                    &promise_done_name,
                )?;
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
            }
        }
        if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_mut() {
            self.sync_async_yield_delegate_snapshot_after_step_result(
                &plan,
                snapshot_bindings,
                property_name.as_str(),
                &step_result_name,
                &promise_done_name,
                &promise_value_name,
                &delegate_completion_name,
                &delegate_iterator_name,
                static_step_result_has_accessor_properties,
            );
        }

        self.finalize_async_yield_delegate_generator_outcome(
            &plan,
            property_name.as_str(),
            &step_result_name,
            &promise_done_name,
            &promise_value_name,
            &delegate_completion_expression,
            &binding_name,
            current_static_index,
            delegate_snapshot_bindings,
            &scoped_snapshot_names,
            static_step_result_has_accessor_properties,
        )
    }
}
