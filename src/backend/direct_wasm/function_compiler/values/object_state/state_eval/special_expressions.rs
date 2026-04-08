use super::super::*;
use super::FunctionStaticEvalContext;

impl StaticSpecialExpressionSource for FunctionStaticEvalContext<'_, '_> {
    fn static_evaluate_special_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        let Expression::SuperCall { callee, arguments } = expression else {
            return None;
        };
        if matches!(
            environment
                .local_bindings
                .get(FunctionCompiler::STATIC_NEW_THIS_INITIALIZED_BINDING),
            Some(Expression::Bool(true))
        ) {
            return None;
        }
        let evaluated_arguments = arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) => self
                    .evaluate_expression_with_state(expression, environment)
                    .map(CallArgument::Expression),
                CallArgument::Spread(expression) => self
                    .evaluate_expression_with_state(expression, environment)
                    .map(CallArgument::Spread),
            })
            .collect::<Option<Vec<_>>>()?;
        let LocalFunctionBinding::User(function_name) = self.resolve_function_binding(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        let current_this_binding = environment
            .object_binding(FunctionCompiler::STATIC_NEW_THIS_BINDING)
            .cloned()
            .unwrap_or_else(empty_object_value_binding);
        let capture_source_bindings = self.resolve_constructor_capture_source_bindings(callee);
        let next_this_binding = self.resolve_user_constructor_object_binding(
            user_function,
            &evaluated_arguments,
            capture_source_bindings.as_ref(),
            current_this_binding,
        )?;
        environment.set_local_object_binding(
            FunctionCompiler::STATIC_NEW_THIS_BINDING.to_string(),
            next_this_binding,
        );
        environment.set_local_binding(
            FunctionCompiler::STATIC_NEW_THIS_INITIALIZED_BINDING.to_string(),
            Expression::Bool(true),
        );
        Some(Expression::Identifier(
            FunctionCompiler::STATIC_NEW_THIS_BINDING.to_string(),
        ))
    }
}
