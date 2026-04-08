use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_enumerate_keys_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        self.emit_numeric_expression(expression)?;
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_get_iterator_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        let materialized_expression = self.materialize_static_expression(expression);
        let iterator_target = if !static_expression_matches(&materialized_expression, expression) {
            &materialized_expression
        } else {
            expression
        };
        if let Expression::Identifier(name) = expression {
            if self
                .state
                .speculation
                .static_semantics
                .has_local_typed_array_view_binding(name)
            {
                if let Some(oob_local) = self
                    .state
                    .speculation
                    .static_semantics
                    .runtime_typed_array_oob_local(name)
                {
                    self.push_local_get(oob_local);
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_named_error_throw("TypeError")?;
                    self.state.emission.output.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                return Ok(());
            }
        }
        if let Some((function_name, returned_expression, effect_statements)) =
            self.analyze_effectful_iterator_source_call(iterator_target)
        {
            self.with_named_function_execution_context(function_name, |compiler| {
                for statement in &effect_statements {
                    compiler.emit_statement(statement)?;
                }
                Ok(())
            })?;
            return self
                .emit_numeric_expression(&Expression::GetIterator(Box::new(returned_expression)));
        }
        if matches!(
            self.infer_value_kind(iterator_target),
            Some(StaticValueKind::Undefined | StaticValueKind::Null)
        ) {
            return self.emit_named_error_throw("TypeError");
        }
        if matches!(
            self.resolve_iterator_source_kind(iterator_target),
            Some(IteratorSourceKind::SimpleGenerator { .. })
        ) {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        let has_next_method = self
            .resolve_object_binding_from_expression(iterator_target)
            .and_then(|object_binding| {
                object_binding_lookup_value(
                    &object_binding,
                    &Expression::String("next".to_string()),
                )
                .cloned()
            })
            .and_then(|value| self.resolve_function_binding_from_expression(&value))
            .is_some()
            || self
                .resolve_member_function_binding(
                    iterator_target,
                    &Expression::String("next".to_string()),
                )
                .is_some();
        if has_next_method {
            self.emit_numeric_expression(iterator_target)?;
            return Ok(());
        }
        let iterator_property = self.materialize_static_expression(&symbol_iterator_expression());
        if self
            .resolve_member_function_binding(iterator_target, &iterator_property)
            .is_some()
            || self
                .resolve_member_getter_binding(iterator_target, &iterator_property)
                .is_some()
        {
            return self.emit_numeric_expression(&Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(iterator_target.clone()),
                    property: Box::new(iterator_property),
                }),
                arguments: Vec::new(),
            });
        }
        self.emit_numeric_expression(iterator_target)?;
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_iterator_close_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        let return_property = Expression::String("return".to_string());
        let capture_source_bindings =
            self.resolve_member_function_capture_source_bindings(expression, &return_property);
        let should_call_return = self
            .resolve_object_binding_from_expression(expression)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &return_property).cloned()
            })
            .map(|value| !matches!(value, Expression::Undefined | Expression::Null))
            .unwrap_or_else(|| {
                self.resolve_member_function_binding(expression, &return_property)
                    .is_some()
                    || self
                        .resolve_member_getter_binding(expression, &return_property)
                        .is_some()
            });
        if let Expression::Identifier(name) = expression
            && let Some(iterator_binding) = self
                .state
                .speculation
                .static_semantics
                .local_array_iterator_binding(name)
                .cloned()
        {
            let state_local = iterator_binding.index_local;
            match iterator_binding.source {
                IteratorSourceKind::SimpleGenerator { steps, .. } => {
                    let closed_state = (steps.len() + 1) as i32;
                    self.push_i32_const(closed_state);
                    self.push_local_set(state_local);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
                IteratorSourceKind::StaticArray { .. }
                | IteratorSourceKind::TypedArrayView { .. }
                | IteratorSourceKind::DirectArguments { .. }
                    if !should_call_return =>
                {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
                _ => {}
            }
        }
        let should_call_return = self
            .resolve_object_binding_from_expression(expression)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &return_property).cloned()
            })
            .map(|value| !matches!(value, Expression::Undefined | Expression::Null))
            .unwrap_or_else(|| {
                self.resolve_member_function_binding(expression, &return_property)
                    .is_some()
                    || self
                        .resolve_member_getter_binding(expression, &return_property)
                        .is_some()
            });
        if should_call_return {
            self.emit_numeric_expression(&Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(expression.clone()),
                    property: Box::new(return_property),
                }),
                arguments: Vec::new(),
            })?;
            self.state.emission.output.instructions.push(0x1a);
            if !capture_source_bindings.is_empty() {
                self.state
                    .runtime
                    .locals
                    .runtime_dynamic_bindings
                    .extend(capture_source_bindings);
            }
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }
        self.emit_numeric_expression(expression)?;
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_await_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        if let Some(outcome) = self.resolve_static_await_resolution_outcome(expression) {
            match outcome {
                StaticEvalOutcome::Value(awaited_value) => {
                    self.emit_numeric_expression(&awaited_value)?;
                }
                StaticEvalOutcome::Throw(throw_value) => {
                    self.emit_static_throw_value(&throw_value)?;
                }
            }
            return Ok(());
        }
        self.emit_numeric_expression(expression)?;
        if let Some(snapshot_result) = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .and_then(|snapshot| {
                self.user_function(&snapshot.function_name)
                    .filter(|function| function.is_async())
                    .and_then(|_| snapshot.result_expression.clone())
            })
        {
            self.state.emission.output.instructions.push(0x1a);
            if let Some(outcome) = self.resolve_static_await_resolution_outcome(&snapshot_result) {
                return self.emit_static_eval_outcome(&outcome);
            }
            return self.emit_numeric_expression(&snapshot_result);
        }
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }
}
