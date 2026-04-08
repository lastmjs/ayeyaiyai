use super::*;

#[path = "static_call_results/binding_results.rs"]
mod binding_results;
#[path = "static_call_results/member_builtins.rs"]
mod member_builtins;
#[path = "static_call_results/specialized_results.rs"]
mod specialized_results;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_call_result_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        self.resolve_static_call_result_expression_with_context(
            callee,
            arguments,
            self.current_function_name(),
        )
        .map(|(value, _)| value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_call_result_expression_with_context(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<(Expression, Option<String>)> {
        self.resolve_specialized_static_call_result_with_context(callee, arguments)
            .or_else(|| {
                self.resolve_static_member_builtin_call_result_with_context(
                    callee,
                    arguments,
                    current_function_name,
                )
            })
            .or_else(|| {
                self.resolve_static_binding_call_result_with_context(
                    callee,
                    arguments,
                    current_function_name,
                )
            })
    }
}
