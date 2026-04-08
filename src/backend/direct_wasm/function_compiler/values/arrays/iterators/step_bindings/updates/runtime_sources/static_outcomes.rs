use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_iterator_step_static_outcome(
        &self,
        iterator_binding: &ArrayIteratorBinding,
        current_static_index: Option<usize>,
        sent_value: &Expression,
    ) -> (Option<bool>, Option<Expression>) {
        match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values,
                keys_only,
                length_local,
                runtime_name,
            } if length_local.is_none() && runtime_name.is_none() => {
                let static_done = current_static_index.map(|index| index >= values.len());
                let static_value = current_static_index.map(|index| {
                    if index >= values.len() {
                        Expression::Undefined
                    } else if *keys_only {
                        Expression::Number(index as f64)
                    } else {
                        values
                            .get(index)
                            .and_then(|value| value.clone())
                            .unwrap_or(Expression::Undefined)
                    }
                });
                (static_done, static_value)
            }
            IteratorSourceKind::SimpleGenerator {
                steps,
                completion_value,
                ..
            } => {
                match current_static_index {
                    Some(index) if index < steps.len() => match &steps[index].outcome {
                        SimpleGeneratorStepOutcome::Yield(value) => (
                            Some(false),
                            Some(self.materialize_static_expression(
                                &Self::substitute_sent_expression(value, sent_value),
                            )),
                        ),
                        SimpleGeneratorStepOutcome::Throw(_) => (None, None),
                    },
                    Some(index) if index == steps.len() => (
                        Some(true),
                        Some(self.materialize_static_expression(
                            &Self::substitute_sent_expression(completion_value, sent_value),
                        )),
                    ),
                    Some(_) => (Some(true), Some(Expression::Undefined)),
                    None => (None, None),
                }
            }
            _ => (None, None),
        }
    }
}
