use super::*;

#[path = "call_frame/aggregate_traversal.rs"]
mod aggregate_traversal;
#[path = "call_frame/direct_bindings.rs"]
mod direct_bindings;
#[path = "call_frame/simple_traversal.rs"]
mod simple_traversal;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn substitute_user_function_call_frame_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Expression {
        let substituted =
            self.substitute_user_function_argument_bindings(expression, user_function, arguments);
        self.substitute_call_frame_special_bindings(
            &substituted,
            user_function,
            this_binding,
            arguments_binding,
        )
    }

    pub(in crate::backend::direct_wasm) fn substitute_call_frame_special_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Expression {
        self.resolve_call_frame_direct_binding_substitution(
            expression,
            user_function,
            this_binding,
            arguments_binding,
        )
        .or_else(|| {
            self.substitute_call_frame_simple_expression(
                expression,
                user_function,
                this_binding,
                arguments_binding,
            )
        })
        .or_else(|| {
            self.substitute_call_frame_aggregate_expression(
                expression,
                user_function,
                this_binding,
                arguments_binding,
            )
        })
        .unwrap_or_else(|| expression.clone())
    }
}
