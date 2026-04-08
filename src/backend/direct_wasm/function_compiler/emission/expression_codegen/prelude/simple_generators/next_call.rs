use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_fresh_simple_generator_next_call(
        &mut self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let iter_result_object = |done: bool, value: Expression| {
            Expression::Object(vec![
                ObjectEntry::Data {
                    key: Expression::String("done".to_string()),
                    value: Expression::Bool(done),
                },
                ObjectEntry::Data {
                    key: Expression::String("value".to_string()),
                    value,
                },
            ])
        };
        let call_expression = Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(object.clone()),
                property: Box::new(Expression::String("next".to_string())),
            }),
            arguments: arguments.to_vec(),
        };
        if let Some(outcome) =
            self.consume_simple_async_generator_next_promise_outcome(object, arguments)?
        {
            self.state
                .speculation
                .static_semantics
                .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
                function_name: "__ayy_simple_async_generator_next".to_string(),
                source_expression: Some(call_expression),
                result_expression: match &outcome {
                    StaticEvalOutcome::Value(value) => Some(value.clone()),
                    StaticEvalOutcome::Throw(_) => None,
                },
                updated_bindings: HashMap::new(),
            });
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }
        let Some((_, steps, completion_effects, completion_value)) =
            self.simple_generator_source_metadata(object).or_else(|| {
                self.resolve_array_prototype_simple_generator_source(object)
                    .map(|(steps, completion_effects, completion_value)| {
                        (false, steps, completion_effects, completion_value)
                    })
            })
        else {
            return Ok(false);
        };
        let Expression::Identifier(object_name) = object else {
            return Ok(false);
        };
        let binding_name = self
            .resolve_local_array_iterator_binding_name(object_name)
            .unwrap_or_else(|| object_name.clone());
        let Some(current_index) = self
            .state
            .speculation
            .static_semantics
            .local_array_iterator_binding(&binding_name)
            .and_then(|binding| binding.static_index)
        else {
            return Ok(false);
        };
        let set_binding_index = |compiler: &mut Self, next_index: usize| {
            if let Some(binding) = compiler
                .state
                .speculation
                .static_semantics
                .local_array_iterator_binding_mut(&binding_name)
            {
                binding.static_index = Some(next_index);
            }
        };
        let sent_value = arguments
            .first()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.materialize_static_expression(expression)
                }
            })
            .unwrap_or(Expression::Undefined);

        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);

        if let Some(step) = steps.get(current_index) {
            let substituted_effects = step
                .effects
                .iter()
                .map(|effect| Self::substitute_sent_statement(effect, &sent_value))
                .collect::<Vec<_>>();
            self.register_bindings(&substituted_effects)?;
            self.sync_visible_runtime_bindings_for_statements(&substituted_effects)?;
            for effect in &substituted_effects {
                self.emit_statement(effect)?;
            }
            match &step.outcome {
                SimpleGeneratorStepOutcome::Yield(value) => {
                    set_binding_index(self, current_index.saturating_add(1));
                    let yielded_value = Self::substitute_sent_expression(value, &sent_value);
                    self.state
                        .speculation
                        .static_semantics
                        .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
                        function_name: "__ayy_simple_generator_next".to_string(),
                        source_expression: Some(call_expression.clone()),
                        result_expression: Some(iter_result_object(false, yielded_value)),
                        updated_bindings: HashMap::new(),
                    });
                    self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                    Ok(true)
                }
                SimpleGeneratorStepOutcome::Throw(value) => {
                    set_binding_index(self, steps.len().saturating_add(1));
                    self.state
                        .speculation
                        .static_semantics
                        .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
                        function_name: "__ayy_simple_generator_next".to_string(),
                        source_expression: Some(call_expression.clone()),
                        result_expression: None,
                        updated_bindings: HashMap::new(),
                    });
                    self.emit_statement(&Statement::Throw(value.clone()))?;
                    Ok(true)
                }
            }
        } else {
            let completion_result_expression = if current_index == steps.len() {
                iter_result_object(true, self.materialize_static_expression(&completion_value))
            } else {
                iter_result_object(true, Expression::Undefined)
            };
            let next_index = if current_index >= steps.len() {
                steps.len().saturating_add(1)
            } else {
                current_index.saturating_add(1)
            };
            set_binding_index(self, next_index);
            if current_index == steps.len() {
                let substituted_completion_effects = completion_effects
                    .iter()
                    .map(|effect| Self::substitute_sent_statement(effect, &sent_value))
                    .collect::<Vec<_>>();
                self.register_bindings(&substituted_completion_effects)?;
                self.sync_visible_runtime_bindings_for_statements(&substituted_completion_effects)?;
                for effect in &substituted_completion_effects {
                    self.emit_statement(effect)?;
                }
            }
            self.state
                .speculation
                .static_semantics
                .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
                function_name: "__ayy_simple_generator_next".to_string(),
                source_expression: Some(call_expression),
                result_expression: Some(completion_result_expression),
                updated_bindings: HashMap::new(),
            });
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            Ok(true)
        }
    }
}
