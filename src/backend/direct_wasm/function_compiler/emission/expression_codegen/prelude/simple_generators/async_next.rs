use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn consume_simple_async_generator_next_promise_outcome(
        &mut self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<Option<StaticEvalOutcome>> {
        let Some((is_async, steps, completion_effects, completion_value)) =
            self.simple_generator_source_metadata(object)
        else {
            return Ok(None);
        };
        if !is_async {
            return Ok(None);
        }
        let sent_value = arguments
            .first()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.materialize_static_expression(expression)
                }
            })
            .unwrap_or(Expression::Undefined);

        let binding_name = if let Expression::Identifier(name) = object {
            self.resolve_local_array_iterator_binding_name(name)
                .unwrap_or_else(|| name.clone())
        } else {
            return Ok(None);
        };
        let Some(current_index) = self
            .state
            .speculation
            .static_semantics
            .local_array_iterator_binding(&binding_name)
            .and_then(|binding| binding.static_index)
        else {
            return Ok(None);
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

        if let Some(step) = steps.get(current_index) {
            let substituted_effects = step
                .effects
                .iter()
                .map(|effect| Self::substitute_sent_statement(effect, &sent_value))
                .collect::<Vec<_>>();
            self.sync_visible_runtime_bindings_for_statements(&substituted_effects)?;
            for effect in &substituted_effects {
                self.emit_statement(effect)?;
            }
            return Ok(Some(match &step.outcome {
                SimpleGeneratorStepOutcome::Yield(value) => {
                    let yielded_value = Self::substitute_sent_expression(value, &sent_value);
                    let yielded_value =
                        match self.resolve_static_await_resolution_outcome(&yielded_value) {
                            Some(StaticEvalOutcome::Throw(throw_value)) => {
                                set_binding_index(self, steps.len().saturating_add(1));
                                return Ok(Some(StaticEvalOutcome::Throw(throw_value)));
                            }
                            Some(StaticEvalOutcome::Value(awaited_value)) => awaited_value,
                            None => value.clone(),
                        };
                    set_binding_index(self, current_index.saturating_add(1));
                    StaticEvalOutcome::Value(Expression::Object(vec![
                        ObjectEntry::Data {
                            key: Expression::String("done".to_string()),
                            value: Expression::Bool(false),
                        },
                        ObjectEntry::Data {
                            key: Expression::String("value".to_string()),
                            value: yielded_value,
                        },
                    ]))
                }
                SimpleGeneratorStepOutcome::Throw(value) => {
                    set_binding_index(self, steps.len().saturating_add(1));
                    StaticEvalOutcome::Throw(StaticThrowValue::Value(value.clone()))
                }
            }));
        }

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
            self.sync_visible_runtime_bindings_for_statements(&substituted_completion_effects)?;
            for effect in &substituted_completion_effects {
                self.emit_statement(effect)?;
            }
        }
        Ok(Some(StaticEvalOutcome::Value(Expression::Object(vec![
            ObjectEntry::Data {
                key: Expression::String("done".to_string()),
                value: Expression::Bool(true),
            },
            ObjectEntry::Data {
                key: Expression::String("value".to_string()),
                value: if current_index == steps.len() {
                    Self::substitute_sent_expression(&completion_value, &sent_value)
                } else {
                    Expression::Undefined
                },
            },
        ]))))
    }
}
