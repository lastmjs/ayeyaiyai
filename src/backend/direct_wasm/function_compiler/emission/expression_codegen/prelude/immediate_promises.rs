use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn promise_handler_expression(
        &self,
        argument: Option<&CallArgument>,
    ) -> Option<Expression> {
        let expression = match argument? {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => expression,
        };
        let materialized = self.materialize_static_expression(expression);
        let effective = if !static_expression_matches(&materialized, expression) {
            materialized
        } else {
            expression.clone()
        };
        (!matches!(effective, Expression::Undefined | Expression::Null)).then_some(effective)
    }

    fn emit_immediate_promise_callback(
        &mut self,
        callback: &Expression,
        argument: &Expression,
        allow_inline: bool,
    ) -> DirectResult<()> {
        let materialized_callback = self.materialize_static_expression(callback);
        let effective_callback = if !static_expression_matches(&materialized_callback, callback) {
            materialized_callback
        } else {
            callback.clone()
        };
        let materialized_argument = self.materialize_static_expression(argument);
        let effective_argument = if !static_expression_matches(&materialized_argument, argument) {
            materialized_argument
        } else {
            argument.clone()
        };
        if let Some(user_function) = self
            .resolve_user_function_from_expression(&effective_callback)
            .cloned()
        {
            self.clear_global_throw_state();
            let bound_capture_slots =
                self.resolve_function_expression_capture_slots(&effective_callback);
            if allow_inline
                && bound_capture_slots.is_some()
                && self.emit_inline_user_function_summary_with_arguments(
                    &user_function,
                    std::slice::from_ref(&effective_argument),
                )?
            {
                self.state.emission.output.instructions.push(0x1a);
                return Ok(());
            }
            if allow_inline
                && bound_capture_slots.is_none()
                && self.can_inline_user_function_call_with_explicit_call_frame(
                    &user_function,
                    std::slice::from_ref(&effective_argument),
                    &Expression::Undefined,
                )
            {
                let result_local = self.allocate_temp_local();
                if self.emit_inline_user_function_summary_with_explicit_call_frame(
                    &user_function,
                    std::slice::from_ref(&effective_argument),
                    &Expression::Undefined,
                    result_local,
                )? {
                    self.push_local_get(result_local);
                    self.state.emission.output.instructions.push(0x1a);
                    return Ok(());
                }
            }
            let callback_arguments = vec![CallArgument::Expression(effective_argument.clone())];
            if let Some(bound_capture_slots) = bound_capture_slots.as_ref() {
                if allow_inline {
                    self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                        &user_function,
                        &callback_arguments,
                        JS_UNDEFINED_TAG,
                        &Expression::Undefined,
                        bound_capture_slots,
                    )?;
                } else {
                    self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_without_static_snapshot(
                        &user_function,
                        &callback_arguments,
                        JS_UNDEFINED_TAG,
                        &Expression::Undefined,
                        bound_capture_slots,
                    )?;
                }
            } else {
                self.emit_user_function_call(&user_function, &callback_arguments)?;
            }
        } else {
            self.emit_numeric_expression(&Expression::Call {
                callee: Box::new(effective_callback),
                arguments: vec![CallArgument::Expression(effective_argument)],
            })?;
        }
        self.state.emission.output.instructions.push(0x1a);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn consume_immediate_promise_outcome(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<Option<StaticEvalOutcome>> {
        if let Some(snapshot_result) = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .filter(|snapshot| {
                snapshot
                    .source_expression
                    .as_ref()
                    .is_some_and(|source| static_expression_matches(source, expression))
            })
            .and_then(|snapshot| {
                self.user_function(&snapshot.function_name)
                    .filter(|function| function.is_async())
                    .and_then(|_| snapshot.result_expression.as_ref())
            })
        {
            return Ok(Some(
                self.resolve_static_await_resolution_outcome(snapshot_result)
                    .unwrap_or(StaticEvalOutcome::Value(snapshot_result.clone())),
            ));
        }
        if let Some(snapshot_result) = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .filter(|snapshot| snapshot.function_name == "__ayy_simple_async_generator_next")
            .and_then(|snapshot| {
                snapshot
                    .source_expression
                    .as_ref()
                    .filter(|source| static_expression_matches(source, expression))
                    .and_then(|_| snapshot.result_expression.as_ref())
            })
        {
            return Ok(Some(StaticEvalOutcome::Value(snapshot_result.clone())));
        }
        if let Some(outcome) = self.consume_immediate_promise_outcome_unmaterialized(expression)? {
            return Ok(Some(outcome));
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.consume_immediate_promise_outcome(&materialized);
        }
        Ok(None)
    }

    fn consume_immediate_promise_outcome_unmaterialized(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<Option<StaticEvalOutcome>> {
        let Expression::Call { callee, arguments } = expression else {
            return Ok(None);
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return Ok(None);
        };
        let Expression::String(property_name) = property.as_ref() else {
            return Ok(None);
        };
        let handlers_require_runtime_chain = arguments.iter().any(|argument| match argument {
            CallArgument::Expression(handler) | CallArgument::Spread(handler) => {
                self.promise_handler_requires_runtime_chain(handler)
            }
        });
        let disable_callback_inlining =
            matches!(property_name.as_str(), "then" | "catch") && handlers_require_runtime_chain;

        match property_name.as_str() {
            "next" | "return" | "throw" => self
                .consume_async_yield_delegate_generator_promise_outcome(
                    object,
                    property_name,
                    arguments,
                )
                .and_then(|outcome| {
                    if outcome.is_some() {
                        Ok(outcome)
                    } else if property_name == "next" {
                        self.consume_simple_async_generator_next_promise_outcome(object, arguments)
                    } else {
                        Ok(None)
                    }
                }),
            "then" | "catch" => {
                let Some(base_outcome) = self.consume_immediate_promise_outcome(object)? else {
                    return Ok(None);
                };

                let (selected_handler, passthrough_outcome) =
                    match (property_name.as_str(), base_outcome) {
                        ("then", StaticEvalOutcome::Value(value)) => (
                            self.promise_handler_expression(arguments.first()),
                            StaticEvalOutcome::Value(value),
                        ),
                        ("then", StaticEvalOutcome::Throw(throw_value)) => (
                            self.promise_handler_expression(arguments.get(1)),
                            StaticEvalOutcome::Throw(throw_value),
                        ),
                        ("catch", StaticEvalOutcome::Value(value)) => {
                            (None, StaticEvalOutcome::Value(value))
                        }
                        ("catch", StaticEvalOutcome::Throw(throw_value)) => (
                            self.promise_handler_expression(arguments.first()),
                            StaticEvalOutcome::Throw(throw_value),
                        ),
                        _ => unreachable!("filtered above"),
                    };

                let Some(handler) = selected_handler else {
                    return Ok(Some(passthrough_outcome));
                };

                let handler_argument = match &passthrough_outcome {
                    StaticEvalOutcome::Value(value) => value,
                    StaticEvalOutcome::Throw(throw_value) => {
                        let Some(value) = self.resolve_static_throw_value_expression(throw_value)
                        else {
                            return Ok(None);
                        };
                        self.emit_immediate_promise_callback(
                            &handler,
                            &value,
                            !disable_callback_inlining,
                        )?;
                        return Ok(Some(StaticEvalOutcome::Value(Expression::Undefined)));
                    }
                };
                self.emit_immediate_promise_callback(
                    &handler,
                    handler_argument,
                    !disable_callback_inlining,
                )?;
                Ok(Some(StaticEvalOutcome::Value(Expression::Undefined)))
            }
            _ => Ok(None),
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_immediate_promise_member_call(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };
        if property_name != "then" && property_name != "catch" {
            return Ok(false);
        }
        let Some(_outcome) = self.consume_immediate_promise_outcome(&Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(object.clone()),
                property: Box::new(property.clone()),
            }),
            arguments: arguments.to_vec(),
        })?
        else {
            if Self::call_is_promise_like_chain(object) {
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                return Ok(true);
            }
            if let Expression::Call { callee, .. } = object
                && let Expression::Member {
                    object: iterator_expression,
                    property: iterator_property,
                } = callee.as_ref()
                && matches!(
                    iterator_property.as_ref(),
                    Expression::String(name) if name == "next"
                )
                && self.is_async_generator_iterator_expression(iterator_expression)
            {
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                return Ok(true);
            }
            return Ok(false);
        };
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }
}
