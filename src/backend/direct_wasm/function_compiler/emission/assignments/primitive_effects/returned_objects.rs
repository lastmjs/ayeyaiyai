use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_effectful_returned_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let (callee, arguments) = match expression {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };
        let binding = self.resolve_function_binding_from_expression(callee)?;
        let argument_expressions = arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) => Some(expression.clone()),
                CallArgument::Spread(_) => None,
            })
            .collect::<Option<Vec<_>>>()?;
        self.resolve_function_binding_static_return_object_binding(&binding, &argument_expressions)
            .or_else(|| {
                match self.resolve_terminal_function_outcome_from_binding(
                    &binding,
                    &argument_expressions,
                )? {
                    StaticEvalOutcome::Value(expression) => {
                        self.resolve_object_binding_from_expression(&expression)
                    }
                    StaticEvalOutcome::Throw(_) => None,
                }
            })
    }
}
