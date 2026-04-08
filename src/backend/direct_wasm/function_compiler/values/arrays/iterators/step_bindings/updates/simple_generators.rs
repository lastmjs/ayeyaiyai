use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_previous_simple_generator_iterator_step(
        &mut self,
        iterator_binding: &mut ArrayIteratorBinding,
        index: usize,
        sent_value: &Expression,
        done_local: u32,
        value_local: u32,
    ) -> (Option<bool>, Option<Expression>) {
        let IteratorSourceKind::SimpleGenerator {
            steps,
            completion_value,
            ..
        } = &iterator_binding.source
        else {
            unreachable!("simple generator helper only called for generator sources");
        };

        let (static_done, static_value) = match steps.get(index).map(|step| &step.outcome) {
            Some(SimpleGeneratorStepOutcome::Yield(value)) => (
                Some(false),
                Some(
                    self.materialize_static_expression(&Self::substitute_sent_expression(
                        value, sent_value,
                    )),
                ),
            ),
            Some(SimpleGeneratorStepOutcome::Throw(_)) => (None, None),
            None if index == steps.len() => (
                Some(true),
                Some(
                    self.materialize_static_expression(&Self::substitute_sent_expression(
                        completion_value,
                        sent_value,
                    )),
                ),
            ),
            None => (Some(true), Some(Expression::Undefined)),
        };

        match steps.get(index).map(|step| &step.outcome) {
            Some(SimpleGeneratorStepOutcome::Yield(value)) => {
                self.push_i32_const(0);
                self.push_local_set(done_local);
                self.emit_numeric_expression(&Self::substitute_sent_expression(value, sent_value))
                    .expect("simple generator yields should be compilable");
                self.push_local_set(value_local);
                self.push_i32_const((index + 1) as i32);
                self.push_local_set(iterator_binding.index_local);
            }
            Some(SimpleGeneratorStepOutcome::Throw(_)) => {
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.push_i32_const((steps.len() + 1) as i32);
                self.push_local_set(iterator_binding.index_local);
            }
            None if index == steps.len() => {
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.emit_numeric_expression(&Self::substitute_sent_expression(
                    completion_value,
                    sent_value,
                ))
                .expect("simple generator completion values should be compilable");
                self.push_local_set(value_local);
                self.push_i32_const((steps.len() + 1) as i32);
                self.push_local_set(iterator_binding.index_local);
            }
            None => {
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.push_i32_const((steps.len() + 1) as i32);
                self.push_local_set(iterator_binding.index_local);
            }
        }

        (static_done, static_value)
    }
}
