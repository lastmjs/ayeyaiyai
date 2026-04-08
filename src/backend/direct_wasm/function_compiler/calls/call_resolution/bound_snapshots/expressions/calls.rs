use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn evaluate_bound_snapshot_call_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(name) if name == "push")
        {
            return self.apply_bound_snapshot_array_push(
                object,
                arguments,
                bindings,
                current_function_name,
            );
        }
        let resolved_callee = if matches!(callee, Expression::Identifier(_)) {
            self.evaluate_bound_snapshot_expression(callee, bindings, current_function_name)
        } else {
            None
        };
        if let Some(Expression::Identifier(marker)) = resolved_callee.as_ref() {
            let stored_value = arguments
                .first()
                .and_then(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => self
                        .evaluate_bound_snapshot_expression(
                            expression,
                            bindings,
                            current_function_name,
                        ),
                })
                .unwrap_or(Expression::Undefined);
            match marker.as_str() {
                SNAPSHOT_AWAIT_RESOLVE_BINDING => {
                    bindings.insert(SNAPSHOT_AWAIT_RESOLUTION_VALUE.to_string(), stored_value);
                    return Some(Expression::Undefined);
                }
                SNAPSHOT_AWAIT_REJECT_BINDING => {
                    bindings.insert(SNAPSHOT_AWAIT_REJECTION_VALUE.to_string(), stored_value);
                    return Some(Expression::Undefined);
                }
                _ => {}
            }
        }
        let binding = self.resolve_function_binding_from_expression_with_context(
            resolved_callee.as_ref().unwrap_or(callee),
            current_function_name,
        )?;
        let evaluated_arguments = arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => self
                    .evaluate_bound_snapshot_expression(
                        expression,
                        bindings,
                        current_function_name,
                    ),
            })
            .collect::<Option<Vec<_>>>()?;
        let (result, updated_bindings) = self
            .resolve_bound_snapshot_function_result_with_arguments(
                &binding,
                bindings,
                &evaluated_arguments,
            )?;
        *bindings = updated_bindings;
        Some(result)
    }
}
