use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn update_runtime_iterator_step_simple_generator(
        &mut self,
        iterator_binding: &mut ArrayIteratorBinding,
        current_static_index: Option<usize>,
        current_index_local: u32,
        sent_value: &Expression,
        done_local: u32,
        value_local: u32,
    ) {
        let IteratorSourceKind::SimpleGenerator {
            steps,
            completion_effects,
            completion_value,
            ..
        } = &iterator_binding.source
        else {
            unreachable!("filtered by caller")
        };
        let mut cumulative_metadata_effects = Vec::new();
        iterator_binding.static_index = current_static_index.map(|index| {
            if index >= steps.len() {
                steps.len().saturating_add(1)
            } else {
                index.saturating_add(1)
            }
        });
        let mut open_frames = 0;
        for (index, step) in steps.iter().enumerate() {
            self.push_local_get(current_index_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)
                .expect("generator iterator comparisons are supported");
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            open_frames += 1;
            let substituted_effects = step
                .effects
                .iter()
                .map(|effect| Self::substitute_sent_statement(effect, sent_value))
                .collect::<Vec<_>>();
            if current_static_index == Some(index) {
                self.register_bindings(&substituted_effects)
                    .expect("simple generator effect bindings should register");
                for effect in &substituted_effects {
                    self.emit_statement(effect)
                        .expect("simple generator effects should be compilable");
                }
            } else {
                self.with_restored_static_binding_metadata(|compiler| {
                    compiler.register_bindings(&cumulative_metadata_effects)?;
                    for prior_effect in &cumulative_metadata_effects {
                        compiler
                            .object_model_domain()
                            .sync_statement_tracking_effects(prior_effect);
                    }
                    compiler.register_bindings(&substituted_effects)?;
                    for effect in &substituted_effects {
                        compiler.emit_statement(effect)?;
                    }
                    Ok(())
                })
                .expect("simple generator effects should be compilable");
            }
            match &step.outcome {
                SimpleGeneratorStepOutcome::Yield(value) => {
                    self.push_i32_const(0);
                    self.push_local_set(done_local);
                    let substituted_value = Self::substitute_sent_expression(value, sent_value);
                    self.emit_numeric_expression(&substituted_value)
                        .expect("simple generator yields should be compilable");
                    self.push_local_set(value_local);
                    self.push_i32_const((index + 1) as i32);
                    self.push_local_set(iterator_binding.index_local);
                }
                SimpleGeneratorStepOutcome::Throw(value) => {
                    self.push_i32_const(1);
                    self.push_local_set(done_local);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(value_local);
                    self.push_i32_const((steps.len() + 1) as i32);
                    self.push_local_set(iterator_binding.index_local);
                    self.emit_statement(&Statement::Throw(Self::substitute_sent_expression(
                        value, sent_value,
                    )))
                    .expect("simple generator throw steps should be compilable");
                }
            }
            self.state.emission.output.instructions.push(0x05);
            cumulative_metadata_effects.extend(substituted_effects);
        }

        self.push_local_get(current_index_local);
        self.push_i32_const(steps.len() as i32);
        self.push_binary_op(BinaryOp::Equal)
            .expect("generator completion comparisons are supported");
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        let substituted_completion_effects = completion_effects
            .iter()
            .map(|effect| Self::substitute_sent_statement(effect, sent_value))
            .collect::<Vec<_>>();
        if current_static_index == Some(steps.len()) {
            self.register_bindings(&substituted_completion_effects)
                .expect("simple generator completion bindings should register");
            for effect in &substituted_completion_effects {
                self.emit_statement(effect)
                    .expect("simple generator completion effects should be compilable");
            }
        } else {
            self.with_restored_static_binding_metadata(|compiler| {
                compiler.register_bindings(&cumulative_metadata_effects)?;
                for prior_effect in &cumulative_metadata_effects {
                    compiler
                        .object_model_domain()
                        .sync_statement_tracking_effects(prior_effect);
                }
                compiler.register_bindings(&substituted_completion_effects)?;
                for effect in &substituted_completion_effects {
                    compiler.emit_statement(effect)?;
                }
                Ok(())
            })
            .expect("simple generator completion effects should be compilable");
        }
        self.push_i32_const(1);
        self.push_local_set(done_local);
        let substituted_completion_value =
            Self::substitute_sent_expression(completion_value, sent_value);
        self.emit_numeric_expression(&substituted_completion_value)
            .expect("simple generator completion values should be compilable");
        self.push_local_set(value_local);
        self.push_i32_const((steps.len() + 1) as i32);
        self.push_local_set(iterator_binding.index_local);
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(1);
        self.push_local_set(done_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_local_set(value_local);
        self.push_i32_const((steps.len() + 1) as i32);
        self.push_local_set(iterator_binding.index_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        while open_frames > 0 {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            open_frames -= 1;
        }
    }

    pub(super) fn update_runtime_iterator_step_async_delegate(
        &mut self,
        iterator_binding: &mut ArrayIteratorBinding,
        done_local: u32,
        value_local: u32,
    ) {
        iterator_binding.static_index = None;
        self.push_i32_const(1);
        self.push_local_set(done_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_local_set(value_local);
    }
}
